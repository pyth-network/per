use {
    super::Auth,
    crate::{
        api::bid::{
            process_bid,
            BidResult,
        },
        auction::Bid,
        config::ChainId,
        opportunity::{
            api::{
                OpportunityBid,
                OpportunityId,
                OpportunityParamsWithMetadata,
            },
            service::handle_opportunity_bid::HandleOpportunityBidInput,
        },
        server::{
            EXIT_CHECK_INTERVAL,
            SHOULD_EXIT,
        },
        state::{
            BidId,
            BidStatusWithId,
            StoreNew,
        },
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
    },
    futures::{
        stream::{
            SplitSink,
            SplitStream,
        },
        SinkExt,
        StreamExt,
    },
    serde::{
        Deserialize,
        Serialize,
    },
    std::{
        collections::HashSet,
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
    tokio::sync::broadcast,
    tracing::{
        instrument,
        Instrument,
    },
    utoipa::ToSchema,
};

pub struct WsState {
    pub subscriber_counter: AtomicUsize,
    pub broadcast_sender:   broadcast::Sender<UpdateEvent>,
    pub broadcast_receiver: broadcast::Receiver<UpdateEvent>,
}

#[derive(Deserialize, Clone, ToSchema)]
#[serde(tag = "method", content = "params")]
pub enum ClientMessage {
    #[serde(rename = "subscribe")]
    Subscribe {
        #[schema(value_type = Vec<String>)]
        chain_ids: Vec<ChainId>,
    },
    #[serde(rename = "unsubscribe")]
    Unsubscribe {
        #[schema(value_type = Vec<String>)]
        chain_ids: Vec<ChainId>,
    },
    #[serde(rename = "post_bid")]
    PostBid { bid: Bid },

    #[serde(rename = "post_opportunity_bid")]
    PostOpportunityBid {
        #[schema(value_type = String)]
        opportunity_id:  OpportunityId,
        opportunity_bid: OpportunityBid,
    },
}

#[derive(Deserialize, Clone, ToSchema)]
pub struct ClientRequest {
    id:  String,
    #[serde(flatten)]
    msg: ClientMessage,
}

/// This enum is used to send an update to the client for any subscriptions made
#[derive(Serialize, Clone, ToSchema)]
#[serde(tag = "type")]
pub enum ServerUpdateResponse {
    #[serde(rename = "new_opportunity")]
    NewOpportunity {
        opportunity: OpportunityParamsWithMetadata,
    },
    #[serde(rename = "bid_status_update")]
    BidStatusUpdate { status: BidStatusWithId },
}

#[derive(Serialize, Clone, ToSchema)]
#[serde(untagged)]
pub enum APIResponse {
    BidResult(BidResult),
}
#[derive(Serialize, Clone, ToSchema)]
#[serde(tag = "status", content = "result")]
pub enum ServerResultMessage {
    #[serde(rename = "success")]
    Success(Option<APIResponse>),
    #[serde(rename = "error")]
    Err(String),
}

/// This enum is used to send the result for a specific client request with the same id
/// id is only None when the client message is invalid
#[derive(Serialize, Clone, ToSchema)]
pub struct ServerResultResponse {
    id:     Option<String>,
    #[serde(flatten)]
    result: ServerResultMessage,
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

#[derive(Clone)]
pub enum UpdateEvent {
    NewOpportunity(OpportunityParamsWithMetadata),
    BidStatusUpdate(BidStatusWithId),
}

pub type SubscriberId = usize;

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
}

const PING_INTERVAL_DURATION: Duration = Duration::from_secs(30);

fn ok_response(id: String) -> ServerResultResponse {
    ServerResultResponse {
        id:     Some(id),
        result: ServerResultMessage::Success(None),
    }
}

impl Subscriber {
    pub fn new(
        id: SubscriberId,
        store: Arc<StoreNew>,
        notify_receiver: broadcast::Receiver<UpdateEvent>,
        receiver: SplitStream<WebSocket>,
        sender: SplitSink<WebSocket, Message>,
        auth: Auth,
    ) -> Self {
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

    async fn handle_new_opportunity(
        &mut self,
        opportunity: OpportunityParamsWithMetadata,
    ) -> Result<()> {
        tracing::Span::current().record("name", "new_opportunity");
        if !self.chain_ids.contains(opportunity.get_chain_id()) {
            // Irrelevant update
            return Ok(());
        }
        let message = serde_json::to_string(&ServerUpdateResponse::NewOpportunity { opportunity })?;
        self.sender.send(message.into()).await?;
        Ok(())
    }

    async fn handle_bid_status_update(&mut self, status: BidStatusWithId) -> Result<()> {
        tracing::Span::current().record("name", "bid_status_update");
        if !self.bid_ids.contains(&status.id) {
            // Irrelevant update
            return Ok(());
        }
        let message = serde_json::to_string(&ServerUpdateResponse::BidStatusUpdate { status })?;
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
        };
        if result.is_err() {
            tracing::Span::current().record("result", "error");
        }
        result
    }

    async fn handle_subscribe(
        &mut self,
        id: String,
        chain_ids: Vec<String>,
    ) -> Result<ServerResultResponse, ServerResultResponse> {
        tracing::Span::current().record("name", "handle_subscribe");
        let available_chain_ids: Vec<&ChainId> = self.store.store.chains.keys().collect();
        let not_found_chain_ids: Vec<&ChainId> = chain_ids
            .iter()
            .filter(|chain_id| !available_chain_ids.contains(chain_id))
            .collect();
        // If there is a single chain id that is not found, we don't subscribe to any of the
        // asked correct chain ids and return an error to be more explicit and clear.
        if !not_found_chain_ids.is_empty() {
            Err(ServerResultResponse {
                id:     Some(id),
                result: ServerResultMessage::Err(format!(
                    "Chain id(s) with id(s) {:?} not found",
                    not_found_chain_ids
                )),
            })
        } else {
            self.chain_ids.extend(chain_ids);
            Ok(ok_response(id))
        }
    }

    async fn handle_unsubscribe(
        &mut self,
        id: String,
        chain_ids: Vec<String>,
    ) -> Result<ServerResultResponse, ServerResultResponse> {
        tracing::Span::current().record("name", "unsubscribe");
        self.chain_ids
            .retain(|chain_id| !chain_ids.contains(chain_id));
        Ok(ok_response(id))
    }

    async fn handle_post_bid(
        &mut self,
        id: String,
        bid: Bid,
    ) -> Result<ServerResultResponse, ServerResultResponse> {
        tracing::Span::current().record("name", "post_bid");
        match process_bid(self.store.clone(), bid, self.auth.clone()).await {
            Ok(bid_result) => {
                self.bid_ids.insert(bid_result.id);
                Ok(ServerResultResponse {
                    id:     Some(id.clone()),
                    result: ServerResultMessage::Success(Some(APIResponse::BidResult(
                        bid_result.0,
                    ))),
                })
            }
            Err(e) => Err(ServerResultResponse {
                id:     Some(id),
                result: ServerResultMessage::Err(e.to_status_and_message().1),
            }),
        }
    }

    #[instrument(skip_all)]
    async fn handle_post_opportunity_bid(
        &mut self,
        id: String,
        opportunity_bid: OpportunityBid,
        opportunity_id: OpportunityId,
    ) -> Result<ServerResultResponse, ServerResultResponse> {
        tracing::Span::current().record("name", "post_opportunity_bid");
        match self
            .store
            .opportunity_service_evm
            .handle_opportunity_bid(HandleOpportunityBidInput {
                opportunity_id,
                opportunity_bid,
                initiation_time: OffsetDateTime::now_utc(),
                auth: self.auth.clone(),
            })
            .await
        {
            Ok(bid_result) => {
                self.bid_ids.insert(bid_result);
                Ok(ServerResultResponse {
                    id:     Some(id.clone()),
                    result: ServerResultMessage::Success(Some(APIResponse::BidResult(BidResult {
                        status: "OK".to_string(),
                        id:     bid_result,
                    }))),
                })
            }
            Err(e) => Err(ServerResultResponse {
                id:     Some(id),
                result: ServerResultMessage::Err(e.to_status_and_message().1),
            }),
        }
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

        let response = match maybe_client_message {
            Err(e) => Err(ServerResultResponse {
                id:     None,
                result: ServerResultMessage::Err(e.to_string()),
            }),
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
            },
        };

        if response.is_err() {
            tracing::Span::current().record("result", "error");
        }

        self.sender
            .send(serde_json::to_string(&response.unwrap_or_else(|e| e))?.into())
            .await?;

        Ok(())
    }
}
