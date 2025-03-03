use {
    super::{
        Auth,
        WrappedRouter,
    },
    crate::{
        auction::{
            api::{
                cancel_bid,
                process_bid,
            },
            entities::BidId,
        },
        config::ChainId,
        opportunity::service::handle_opportunity_bid::HandleOpportunityBidInput,
        server::{
            EXIT_CHECK_INTERVAL,
            SHOULD_EXIT,
        },
        state::StoreNew,
    },
    anyhow::{
        anyhow,
        Result,
    },
    axum::{
        extract::{
            ws::{
                Message,
                WebSocket,
            },
            State,
            WebSocketUpgrade,
        },
        response::IntoResponse,
        Router,
    },
    express_relay_api_types::{
        bid::{
            BidCancel,
            BidCreate,
            BidResult,
            BidStatusWithId,
        },
        opportunity::{
            Opportunity,
            OpportunityBidEvm,
            OpportunityDelete,
            OpportunityId,
        },
        ws::{
            APIResponse,
            ClientMessage,
            ClientRequest,
            Route,
            ServerResultMessage,
            ServerResultResponse,
            ServerUpdateResponse,
        },
        SvmChainUpdate,
    },
    futures::{
        stream::{
            SplitSink,
            SplitStream,
        },
        SinkExt,
        StreamExt,
    },
    std::{
        collections::HashSet,
        future::Future,
        sync::{
            atomic::{
                AtomicUsize,
                Ordering,
            },
            Arc,
        },
        time::Duration,
    },
    time::OffsetDateTime,
    tokio::sync::{
        broadcast,
        Semaphore,
    },
    tracing::{
        instrument,
        Instrument,
    },
};

pub struct WsState {
    pub subscriber_counter: AtomicUsize,
    pub broadcast_sender:   broadcast::Sender<UpdateEvent>,
    pub broadcast_receiver: broadcast::Receiver<UpdateEvent>,
}

pub async fn ws_route_handler(
    auth: Auth,
    ws: WebSocketUpgrade,
    State(store): State<Arc<StoreNew>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| websocket_handler(socket, store, auth))
}

async fn websocket_handler(stream: WebSocket, state: Arc<StoreNew>, auth: Auth) {
    let ws_state = &state.store.ws;
    let id = ws_state.subscriber_counter.fetch_add(1, Ordering::SeqCst);
    let (sender, receiver) = stream.split();
    let new_receiver = ws_state.broadcast_receiver.resubscribe();
    let mut subscriber = Subscriber::new(id, state, new_receiver, receiver, sender, auth);
    subscriber.run().await;
}

#[derive(Clone, PartialEq, Debug)]
pub enum UpdateEvent {
    NewOpportunity(Opportunity),
    BidStatusUpdate(BidStatusWithId),
    SvmChainUpdate(SvmChainUpdate),
    RemoveOpportunities(OpportunityDelete),
}

pub type SubscriberId = usize;


#[derive(Debug, Clone)]
struct DeferredResponse {
    response:      ServerResultResponse,
    bid_id_to_add: Option<BidId>,
}

/// Subscriber is an actor that handles a single websocket connection.
/// It listens to the store for updates and sends them to the client.
pub struct Subscriber {
    id:                  SubscriberId,
    closed:              bool,
    store:               Arc<StoreNew>,
    notify_receiver:     broadcast::Receiver<UpdateEvent>,
    receiver:            SplitStream<WebSocket>,
    sender:              SplitSink<WebSocket, Message>,
    chain_ids:           HashSet<ChainId>,
    bid_ids:             HashSet<BidId>,
    ping_interval:       tokio::time::Interval,
    exit_check_interval: tokio::time::Interval,
    responded_to_ping:   bool,
    auth:                Auth,
    active_requests:     Arc<Semaphore>,
    response_sender:     broadcast::Sender<DeferredResponse>,
    response_receiver:   broadcast::Receiver<DeferredResponse>,
}

const PING_INTERVAL_DURATION: Duration = Duration::from_secs(30);

fn ok_response(id: String) -> ServerResultResponse {
    ServerResultResponse {
        id:     Some(id),
        result: ServerResultMessage::Success(None),
    }
}

const MAX_ACTIVE_REQUESTS: usize = 10;

impl Subscriber {
    pub fn new(
        id: SubscriberId,
        store: Arc<StoreNew>,
        notify_receiver: broadcast::Receiver<UpdateEvent>,
        receiver: SplitStream<WebSocket>,
        sender: SplitSink<WebSocket, Message>,
        auth: Auth,
    ) -> Self {
        let (response_sender, response_receiver) = broadcast::channel(100);
        Self {
            id,
            closed: false,
            store,
            notify_receiver,
            receiver,
            sender,
            chain_ids: HashSet::new(),
            bid_ids: HashSet::new(),
            ping_interval: tokio::time::interval(PING_INTERVAL_DURATION),
            exit_check_interval: tokio::time::interval(EXIT_CHECK_INTERVAL),
            responded_to_ping: true, // We start with true so we don't close the connection immediately
            auth,
            active_requests: Arc::new(Semaphore::new(MAX_ACTIVE_REQUESTS)),
            response_receiver,
            response_sender,
        }
    }

    pub async fn run(&mut self) {
        while !self.closed {
            if let Err(e) = self.handle_next().await {
                tracing::debug!(subscriber = self.id, error = ?e, "Error Handling Subscriber Message.");
                break;
            }
        }
    }

    async fn handle_next(&mut self) -> Result<()> {
        tokio::select! {
            maybe_update_event = self.notify_receiver.recv() => {
                match maybe_update_event {

                    Ok(event) => self.handle_update(event).await,
                    Err(e) => Err(anyhow!("Error receiving update event: {:?}", e)),
                }
            },
            maybe_message_or_err = self.receiver.next() => {
                self.handle_client_message(
                    maybe_message_or_err.ok_or(anyhow!("Client channel is closed"))??
                ).await
            },
            response_received = self.response_receiver.recv() => {
                match response_received{
                    Ok(DeferredResponse{response, bid_id_to_add}) =>{
                            if let Some(bid_id) = bid_id_to_add {
                                self.bid_ids.insert(bid_id);
                            }
                            self.sender.send(serde_json::to_string(&response)?.into()).await?

                        },
                    Err(e) => {
                        tracing::warn!(subscriber = self.id, error = ?e, "Error Handling Subscriber Response Message.");
                    }
                };
                Ok(())
            },
            _  = self.ping_interval.tick() => {
                if let Auth::Authorized(token, _) = self.auth.clone() {
                    if self.store.store.get_profile_by_token(&token).await.is_err() {
                        return Err(anyhow!("Invalid token. Closing connection."));
                    }
                }
                if !self.responded_to_ping {
                    return Err(anyhow!("Subscriber did not respond to ping. Closing connection."));
                }
                self.responded_to_ping = false;
                self.sender.send(Message::Ping(vec![])).await?;
                Ok(())
            },
            _ = self.exit_check_interval.tick() => {
                if SHOULD_EXIT.load(Ordering::Acquire) {
                    self.sender.close().await?;
                    self.closed = true;
                    return Err(anyhow!("Application is shutting down. Closing connection."));
                }
                Ok(())
            }
        }
    }

    async fn handle_new_opportunity(&mut self, opportunity: Opportunity) -> Result<()> {
        if !self.chain_ids.contains(opportunity.get_chain_id()) {
            // Irrelevant update
            return Ok(());
        }
        let message = serde_json::to_string(&ServerUpdateResponse::NewOpportunity { opportunity })?;
        self.sender.send(message.into()).await?;
        Ok(())
    }

    async fn handle_bid_status_update(&mut self, status: BidStatusWithId) -> Result<()> {
        if !self.bid_ids.contains(&status.id) {
            // Irrelevant update
            return Ok(());
        }
        let message = serde_json::to_string(&ServerUpdateResponse::BidStatusUpdate { status })?;
        self.sender.send(message.into()).await?;
        Ok(())
    }

    async fn handle_svm_chain_update(&mut self, svm_chain_update: SvmChainUpdate) -> Result<()> {
        if !self.chain_ids.contains(&svm_chain_update.chain_id) {
            // Irrelevant update
            return Ok(());
        }
        let message = serde_json::to_string(&ServerUpdateResponse::SvmChainUpdate {
            update: svm_chain_update,
        })?;
        self.sender.send(message.into()).await?;
        Ok(())
    }

    async fn handle_remove_opportunities(
        &mut self,
        opportunity_delete: OpportunityDelete,
    ) -> Result<()> {
        if !self.chain_ids.contains(opportunity_delete.get_chain_id()) {
            // Irrelevant update
            return Ok(());
        }
        let message = serde_json::to_string(&ServerUpdateResponse::RemoveOpportunities {
            opportunity_delete,
        })?;
        self.sender.send(message.into()).await?;
        Ok(())
    }

    #[instrument(
        target = "metrics",
        fields(category = "ws_update", result = "success", name),
        skip_all
    )]
    async fn handle_update(&mut self, event: UpdateEvent) -> Result<()> {
        let result = match event.clone() {
            UpdateEvent::NewOpportunity(opportunity) => {
                tracing::Span::current().record("name", "new_opportunity");
                self.handle_new_opportunity(opportunity).await
            }
            UpdateEvent::BidStatusUpdate(status) => {
                tracing::Span::current().record("name", "bid_status_update");
                self.handle_bid_status_update(status).await
            }
            UpdateEvent::SvmChainUpdate(svm_chain_update) => {
                tracing::Span::current().record("name", "svm_chain_update");
                self.handle_svm_chain_update(svm_chain_update).await
            }
            UpdateEvent::RemoveOpportunities(opportunity_delete) => {
                tracing::Span::current().record("name", "remove_opportunity");
                self.handle_remove_opportunities(opportunity_delete).await
            }
        };
        if result.is_err() {
            tracing::Span::current().record("result", "error");
        }
        result
    }

    async fn handle_subscribe(&mut self, message_id: String, chain_ids: Vec<String>) {
        let available_chain_ids: Vec<&ChainId> = self
            .store
            .store
            .chains_evm
            .keys()
            .chain(self.store.store.chains_svm.keys())
            .collect();
        let not_found_chain_ids: Vec<&ChainId> = chain_ids
            .iter()
            .filter(|chain_id| !available_chain_ids.contains(chain_id))
            .collect();
        // If there is a single chain id that is not found, we don't subscribe to any of the
        // asked correct chain ids and return an error to be more explicit and clear.
        let resp = if !not_found_chain_ids.is_empty() {
            ServerResultResponse {
                id:     Some(message_id),
                result: ServerResultMessage::Err(format!(
                    "Chain id(s) with id(s) {:?} not found",
                    not_found_chain_ids
                )),
            }
        } else {
            self.chain_ids.extend(chain_ids);
            ok_response(message_id)
        };
        Self::send_response(
            &self.response_sender,
            DeferredResponse {
                response:      resp,
                bid_id_to_add: None,
            },
        );
    }

    async fn handle_unsubscribe(&mut self, message_id: String, chain_ids: Vec<String>) {
        self.chain_ids
            .retain(|chain_id| !chain_ids.contains(chain_id));
        let resp = ok_response(message_id);
        Self::send_response(
            &self.response_sender,
            DeferredResponse {
                response:      resp,
                bid_id_to_add: None,
            },
        );
    }

    fn send_response(
        response_sender: &broadcast::Sender<DeferredResponse>,
        deferred_response: DeferredResponse,
    ) {
        if matches!(
            deferred_response.response.result,
            ServerResultMessage::Err(_)
        ) {
            tracing::Span::current().record("result", "error");
        }
        if let Err(e) = response_sender.send(deferred_response) {
            tracing::warn!(error = ?e, "Error sending response to subscriber");
        }
    }

    async fn spawn_deferred(
        &mut self,
        fut: impl Future<Output = DeferredResponse> + Send + 'static,
    ) {
        let permit = self
            .active_requests
            .clone()
            .acquire_owned()
            .await
            .expect("Semaphore should not be closed");
        let response_sender = self.response_sender.clone();
        self.store.task_tracker.spawn(
            async move {
                let resp = fut.await;
                Self::send_response(&response_sender, resp);
                drop(permit);
            }
            .in_current_span(),
        );
    }

    async fn handle_post_bid(&mut self, message_id: String, bid: BidCreate) {
        let (auth, store) = (self.auth.clone(), self.store.clone());
        self.spawn_deferred(async move {
            match process_bid(auth, store, bid).await {
                Ok(bid_result) => DeferredResponse {
                    bid_id_to_add: Some(bid_result.id),
                    response:      ServerResultResponse {
                        id:     Some(message_id.clone()),
                        result: ServerResultMessage::Success(Some(APIResponse::BidResult(
                            bid_result.0,
                        ))),
                    },
                },
                Err(e) => DeferredResponse {
                    response:      ServerResultResponse {
                        id:     Some(message_id),
                        result: ServerResultMessage::Err(e.to_status_and_message().1),
                    },
                    bid_id_to_add: None,
                },
            }
        })
        .await;
    }

    async fn handle_cancel_bid(&mut self, message_id: String, bid_cancel: BidCancel) {
        let (auth, store) = (self.auth.clone(), self.store.clone());
        self.spawn_deferred(async move {
            let resp = match cancel_bid(auth, store, bid_cancel).await {
                Ok(_) => ok_response(message_id),
                Err(e) => ServerResultResponse {
                    id:     Some(message_id),
                    result: ServerResultMessage::Err(e.to_status_and_message().1),
                },
            };
            DeferredResponse {
                response:      resp,
                bid_id_to_add: None,
            }
        })
        .await;
    }

    #[instrument(skip_all)]
    async fn handle_post_opportunity_bid(
        &mut self,
        message_id: String,
        opportunity_bid: OpportunityBidEvm,
        opportunity_id: OpportunityId,
    ) {
        let store = self.store.clone();
        let auth = self.auth.clone();
        self.spawn_deferred(async move {
            match store
                .opportunity_service_evm
                .handle_opportunity_bid(HandleOpportunityBidInput {
                    opportunity_id,
                    opportunity_bid,
                    initiation_time: OffsetDateTime::now_utc(),
                    auth,
                })
                .await
            {
                Ok(bid_result) => DeferredResponse {
                    response:      ServerResultResponse {
                        id:     Some(message_id.clone()),
                        result: ServerResultMessage::Success(Some(APIResponse::BidResult(
                            BidResult {
                                status: "OK".to_string(),
                                id:     bid_result,
                            },
                        ))),
                    },
                    bid_id_to_add: Some(bid_result),
                },
                Err(e) => DeferredResponse {
                    response:      ServerResultResponse {
                        id:     Some(message_id),
                        result: ServerResultMessage::Err(e.to_status_and_message().1),
                    },
                    bid_id_to_add: None,
                },
            }
        })
        .await;
    }

    #[instrument(
        target = "metrics",
        fields(category = "ws_client_message", result = "success", name),
        skip_all
    )]
    async fn handle_client_message(&mut self, message: Message) -> Result<()> {
        let maybe_client_message = match message {
            Message::Close(_) => {
                // Closing the connection. We don't remove it from the subscribers
                // list, instead when the Subscriber struct is dropped the channel
                // to subscribers list will be closed and it will eventually get
                // removed.
                // Send the close message to gracefully shut down the connection
                // Otherwise the client might get an abnormal Websocket closure
                // error.
                tracing::Span::current().record("name", "close");
                if let Err(e) = self.sender.close().await {
                    tracing::Span::current().record("result", "error");
                    return Err(e.into());
                }
                self.closed = true;
                return Ok(());
            }
            Message::Text(text) => serde_json::from_str::<ClientRequest>(&text),
            Message::Binary(data) => serde_json::from_slice::<ClientRequest>(&data),
            Message::Ping(_) => {
                // Axum will send Pong automatically
                tracing::Span::current().record("name", "ping");
                return Ok(());
            }
            Message::Pong(_) => {
                tracing::Span::current().record("name", "pong");
                self.responded_to_ping = true;
                return Ok(());
            }
        };

        match maybe_client_message {
            Err(e) => {
                let resp = DeferredResponse {
                    response:      ServerResultResponse {
                        id:     None,
                        result: ServerResultMessage::Err(e.to_string()),
                    },
                    bid_id_to_add: None,
                };
                Self::send_response(&self.response_sender, resp);
            }
            Ok(ClientRequest { msg, id }) => match msg {
                ClientMessage::Subscribe { chain_ids } => {
                    tracing::Span::current().record("name", "subscribe");
                    self.handle_subscribe(id, chain_ids).await
                }
                ClientMessage::Unsubscribe { chain_ids } => {
                    tracing::Span::current().record("name", "unsubscribe");
                    self.handle_unsubscribe(id, chain_ids).await
                }
                ClientMessage::PostBid { bid } => {
                    tracing::Span::current().record("name", "post_bid");
                    self.handle_post_bid(id, bid).await
                }
                ClientMessage::PostOpportunityBid {
                    opportunity_bid,
                    opportunity_id,
                } => {
                    tracing::Span::current().record("name", "post_opportunity_bid");
                    self.handle_post_opportunity_bid(id, opportunity_bid, opportunity_id)
                        .in_current_span()
                        .await
                }
                ClientMessage::CancelBid { data } => {
                    tracing::Span::current().record("name", "cancel_bid");
                    self.handle_cancel_bid(id, data).await
                }
            },
        };

        Ok(())
    }
}


pub fn get_routes(store: Arc<StoreNew>) -> Router<Arc<StoreNew>> {
    WrappedRouter::new(store)
        .route(Route::Ws, ws_route_handler)
        .router
}
