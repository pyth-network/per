use {
    crate::{
        api::{
            bid::{
                handle_bid,
                Bid,
            },
            liquidation::OpportunityParamsWithMetadata,
        },
        config::ChainId,
        server::{
            EXIT_CHECK_INTERVAL,
            SHOULD_EXIT,
        },
        state::{
            BidId,
            BidStatus,
            Store,
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
    tokio::sync::broadcast,
    utoipa::ToSchema,
};

pub struct WsState {
    pub subscriber_counter: AtomicUsize,
    pub broadcast_sender:   broadcast::Sender<UpdateEvent>,
    pub broadcast_receiver: broadcast::Receiver<UpdateEvent>,
}

#[derive(Deserialize, Debug, Clone, ToSchema)]
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
    #[serde(rename = "submit_bid")]
    SubmitBid { bid: Bid },
}

#[derive(Deserialize, Debug, Clone, ToSchema)]
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
    BidStatusUpdate { id: BidId, status: BidStatus },
}

#[derive(Serialize, Debug, Clone, ToSchema)]
#[serde(tag = "status", content = "result")]
pub enum ServerResultMessage {
    #[serde(rename = "success")]
    Success,
    #[serde(rename = "error")]
    Err(String),
}

/// This enum is used to send the result for a specific client request with the same id
/// id is only None when the client message is invalid
#[derive(Serialize, Debug, Clone, ToSchema)]
pub struct ServerResultResponse {
    id:     Option<String>,
    #[serde(flatten)]
    result: ServerResultMessage,
}

pub async fn ws_route_handler(
    ws: WebSocketUpgrade,
    State(store): State<Arc<Store>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| websocket_handler(socket, store))
}

async fn websocket_handler(stream: WebSocket, state: Arc<Store>) {
    let ws_state = &state.ws;
    let id = ws_state.subscriber_counter.fetch_add(1, Ordering::SeqCst);
    let (sender, receiver) = stream.split();
    let new_receiver = ws_state.broadcast_receiver.resubscribe();
    let mut subscriber = Subscriber::new(id, state, new_receiver, receiver, sender);
    subscriber.run().await;
}

#[derive(Clone)]
pub enum UpdateEvent {
    NewOpportunity(OpportunityParamsWithMetadata),
    BidStatusUpdate { id: BidId, status: BidStatus },
}

pub type SubscriberId = usize;

/// Subscriber is an actor that handles a single websocket connection.
/// It listens to the store for updates and sends them to the client.
pub struct Subscriber {
    id:                  SubscriberId,
    closed:              bool,
    store:               Arc<Store>,
    notify_receiver:     broadcast::Receiver<UpdateEvent>,
    receiver:            SplitStream<WebSocket>,
    sender:              SplitSink<WebSocket, Message>,
    chain_ids:           HashSet<ChainId>,
    bid_ids:             HashSet<BidId>,
    ping_interval:       tokio::time::Interval,
    exit_check_interval: tokio::time::Interval,
    responded_to_ping:   bool,
}

const PING_INTERVAL_DURATION: Duration = Duration::from_secs(30);

impl Subscriber {
    pub fn new(
        id: SubscriberId,
        store: Arc<Store>,
        notify_receiver: broadcast::Receiver<UpdateEvent>,
        receiver: SplitStream<WebSocket>,
        sender: SplitSink<WebSocket, Message>,
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

    async fn handle_update(&mut self, event: UpdateEvent) -> Result<()> {
        match event.clone() {
            UpdateEvent::NewOpportunity(opportunity) => {
                if !self.chain_ids.contains(opportunity.get_chain_id()) {
                    // Irrelevant update
                    return Ok(());
                }
                let message =
                    serde_json::to_string(&ServerUpdateResponse::NewOpportunity { opportunity })?;
                self.sender.send(message.into()).await?;
            }
            UpdateEvent::BidStatusUpdate { id, status } => {
                if !self.bid_ids.contains(&id) {
                    // Irrelevant update
                    return Ok(());
                }
                let message =
                    serde_json::to_string(&ServerUpdateResponse::BidStatusUpdate { id, status })?;
                self.sender.send(message.into()).await?;
            }
        }

        Ok(())
    }

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
                self.sender.close().await?;
                self.closed = true;
                return Ok(());
            }
            Message::Text(text) => serde_json::from_str::<ClientRequest>(&text),
            Message::Binary(data) => serde_json::from_slice::<ClientRequest>(&data),
            Message::Ping(_) => {
                // Axum will send Pong automatically
                return Ok(());
            }
            Message::Pong(_) => {
                self.responded_to_ping = true;
                return Ok(());
            }
        };

        let response = match maybe_client_message {
            Err(e) => ServerResultResponse {
                id:     None,
                result: ServerResultMessage::Err(e.to_string()),
            },

            Ok(ClientRequest { msg, id }) => {
                let ok_response = ServerResultResponse {
                    id:     Some(id.clone()),
                    result: ServerResultMessage::Success,
                };
                match msg {
                    ClientMessage::Subscribe { chain_ids } => {
                        let available_chain_ids: Vec<&ChainId> = self.store.chains.keys().collect();

                        let not_found_chain_ids: Vec<&ChainId> = chain_ids
                            .iter()
                            .filter(|chain_id| !available_chain_ids.contains(chain_id))
                            .collect();

                        // If there is a single chain id that is not found, we don't subscribe to any of the
                        // asked correct chain ids and return an error to be more explicit and clear.
                        if !not_found_chain_ids.is_empty() {
                            ServerResultResponse {
                                id:     Some(id),
                                result: ServerResultMessage::Err(format!(
                                    "Chain id(s) with id(s) {:?} not found",
                                    not_found_chain_ids
                                )),
                            }
                        } else {
                            self.chain_ids.extend(chain_ids);
                            ok_response
                        }
                    }
                    ClientMessage::Unsubscribe { chain_ids } => {
                        self.chain_ids
                            .retain(|chain_id| !chain_ids.contains(chain_id));
                        ok_response
                    }
                    ClientMessage::SubmitBid { bid } => {
                        match handle_bid(self.store.clone(), bid).await {
                            Ok(bid_id) => {
                                self.bid_ids.insert(bid_id);
                                ok_response
                            }
                            Err(e) => ServerResultResponse {
                                id:     Some(id),
                                result: ServerResultMessage::Err(e.to_status_and_message().1),
                            },
                        }
                    }
                }
            }
        };
        self.sender
            .send(serde_json::to_string(&response)?.into())
            .await?;
        Ok(())
    }
}
