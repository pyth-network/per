use {
    crate::{
        api::{
            liquidation::OpportunityParamsWithMetadata,
            SHOULD_EXIT,
        },
        config::ChainId,
        state::{
            LiquidationOpportunity,
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
        http::HeaderMap,
        response::IntoResponse,
    },
    dashmap::DashMap,
    ethers::types::Chain,
    futures::{
        future::join_all,
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
    tokio::sync::mpsc,
};

pub struct WsState {
    pub subscriber_counter: AtomicUsize,
    pub subscribers:        DashMap<SubscriberId, mpsc::Sender<UpdateEvent>>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(tag = "type")]
enum ClientMessage {
    #[serde(rename = "subscribe")]
    Subscribe { chain_ids: Vec<ChainId> },
    #[serde(rename = "unsubscribe")]
    Unsubscribe { chain_ids: Vec<ChainId> },
}

#[derive(Serialize, Clone)]
#[serde(tag = "type")]
enum ServerMessage {
    #[serde(rename = "response")]
    Response(ServerResponseMessage),
    #[serde(rename = "new_opportunity")]
    NewOpportunity {
        opportunity: OpportunityParamsWithMetadata,
    },
}

#[derive(Serialize, Debug, Clone)]
#[serde(tag = "status")]
enum ServerResponseMessage {
    #[serde(rename = "success")]
    Success,
    #[serde(rename = "error")]
    Err { error: String },
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
    let (notify_sender, notify_receiver) = mpsc::channel(NOTIFICATIONS_CHAN_LEN);
    let (sender, receiver) = stream.split();
    ws_state.subscribers.insert(id, notify_sender);
    let mut subscriber = Subscriber::new(id, state, notify_receiver, receiver, sender);
    subscriber.run().await;
}

#[derive(Clone)]
pub enum UpdateEvent {
    NewOpportunity(OpportunityParamsWithMetadata),
}

pub type SubscriberId = usize;

/// Subscriber is an actor that handles a single websocket connection.
/// It listens to the store for updates and sends them to the client.
pub struct Subscriber {
    id:                  SubscriberId,
    closed:              bool,
    store:               Arc<Store>,
    notify_receiver:     mpsc::Receiver<UpdateEvent>,
    receiver:            SplitStream<WebSocket>,
    sender:              SplitSink<WebSocket, Message>,
    chain_ids:           HashSet<ChainId>,
    ping_interval:       tokio::time::Interval,
    exit_check_interval: tokio::time::Interval,
    responded_to_ping:   bool,
}

const PING_INTERVAL_DURATION: Duration = Duration::from_secs(30);
const NOTIFICATIONS_CHAN_LEN: usize = 1000;

impl Subscriber {
    pub fn new(
        id: SubscriberId,
        store: Arc<Store>,
        notify_receiver: mpsc::Receiver<UpdateEvent>,
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
            ping_interval: tokio::time::interval(PING_INTERVAL_DURATION),
            exit_check_interval: tokio::time::interval(Duration::from_secs(5)),
            responded_to_ping: true, // We start with true so we don't close the connection immediately
        }
    }

    #[tracing::instrument(skip(self))]
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
                    Some(event) => self.handle_update(event).await,
                    None => Err(anyhow!("Update channel closed. This should never happen. Closing connection."))
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
                    serde_json::to_string(&ServerMessage::NewOpportunity { opportunity })?;
                self.sender.send(message.into()).await?;
            }
        }

        Ok(())
    }

    #[tracing::instrument(skip(self, message))]
    async fn handle_client_message(&mut self, message: Message) -> Result<()> {
        let maybe_client_message = match message {
            Message::Close(_) => {
                // Closing the connection. We don't remove it from the subscribers
                // list, instead when the Subscriber struct is dropped the channel
                // to subscribers list will be closed and it will eventually get
                // removed.
                tracing::trace!(id = self.id, "Subscriber Closed Connection.");

                // Send the close message to gracefully shut down the connection
                // Otherwise the client might get an abnormal Websocket closure
                // error.
                self.sender.close().await?;
                self.closed = true;
                return Ok(());
            }
            Message::Text(text) => serde_json::from_str::<ClientMessage>(&text),
            Message::Binary(data) => serde_json::from_slice::<ClientMessage>(&data),
            Message::Ping(_) => {
                // Axum will send Pong automatically
                return Ok(());
            }
            Message::Pong(_) => {
                self.responded_to_ping = true;
                return Ok(());
            }
        };

        match maybe_client_message {
            Err(e) => {
                self.sender
                    .send(
                        serde_json::to_string(&ServerMessage::Response(
                            ServerResponseMessage::Err {
                                error: e.to_string(),
                            },
                        ))?
                        .into(),
                    )
                    .await?;
                return Ok(());
            }

            Ok(ClientMessage::Subscribe { chain_ids }) => {
                let available_chain_ids: Vec<&ChainId> = self.store.chains.keys().collect();

                let not_found_chain_ids: Vec<&ChainId> = chain_ids
                    .iter()
                    .filter(|chain_id| !available_chain_ids.contains(chain_id))
                    .collect();

                // If there is a single chain id that is not found, we don't subscribe to any of the
                // asked correct chain ids and return an error to be more explicit and clear.
                if !not_found_chain_ids.is_empty() {
                    self.sender
                        .send(
                            serde_json::to_string(&ServerMessage::Response(
                                ServerResponseMessage::Err {
                                    error: format!(
                                        "Chain id(s) with id(s) {:?} not found",
                                        not_found_chain_ids
                                    ),
                                },
                            ))?
                            .into(),
                        )
                        .await?;
                    return Ok(());
                } else {
                    self.chain_ids.extend(chain_ids.into_iter());
                }
            }
            Ok(ClientMessage::Unsubscribe { chain_ids }) => {
                self.chain_ids
                    .retain(|chain_id| !chain_ids.contains(chain_id));
            }
        }


        self.sender
            .send(
                serde_json::to_string(&ServerMessage::Response(ServerResponseMessage::Success))?
                    .into(),
            )
            .await?;

        Ok(())
    }
}


pub async fn notify_updates(ws_state: &WsState, event: UpdateEvent) {
    let closed_subscribers: Vec<Option<SubscriberId>> =
        join_all(ws_state.subscribers.iter_mut().map(|subscriber| {
            let event = event.clone();
            async move {
                match subscriber.send(event).await {
                    Ok(_) => None,
                    Err(_) => {
                        // An error here indicates the channel is closed (which may happen either when the
                        // client has sent Message::Close or some other abrupt disconnection). We remove
                        // subscribers only when send fails so we can handle closure only once when we are
                        // able to see send() fail.
                        Some(*subscriber.key())
                    }
                }
            }
        }))
        .await;

    // Remove closed_subscribers from ws_state
    closed_subscribers.into_iter().for_each(|id| {
        if let Some(id) = id {
            ws_state.subscribers.remove(&id);
        }
    });
}
