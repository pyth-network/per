pub use {
    ethers,
    express_relay_api_types as api_types,
};
use {
    ethers::signers::LocalWallet,
    evm::{
        get_config,
        get_params,
        make_adapter_calldata,
        BidParamsEvm,
    },
    express_relay_api_types::{
        bid::{
            BidCreate,
            BidCreateEvm,
        },
        opportunity::{
            GetOpportunitiesQueryParams,
            Opportunity,
            Route,
        },
        ws::ServerResultMessage,
        ErrorBodyResponse,
        Routable,
    },
    futures_util::{
        SinkExt,
        Stream,
        StreamExt,
    },
    reqwest::Response,
    serde::{
        de::DeserializeOwned,
        Deserialize,
        Serialize,
    },
    std::{
        collections::HashMap,
        marker::PhantomData,
        pin::Pin,
        sync::Arc,
        task::{
            Context,
            Poll,
        },
        time::Duration,
    },
    tokio::{
        net::TcpStream,
        sync::{
            broadcast,
            mpsc,
            oneshot,
            RwLock,
        },
    },
    tokio_stream::wrappers::{
        errors::BroadcastStreamRecvError,
        BroadcastStream,
    },
    tokio_tungstenite::{
        connect_async,
        tungstenite::{
            client::IntoClientRequest,
            Message,
        },
        MaybeTlsStream,
        WebSocketStream,
    },
    url::Url,
};

pub mod evm;

pub struct Client {
    http_url: Url,
    ws_url:   Url,
    api_key:  Option<String>,
    client:   reqwest::Client,
}

pub struct ClientConfig {
    pub http_url: String,
    pub api_key:  Option<String>,
}

#[derive(Debug)]
pub enum ClientError {
    InvalidHttpUrl(String),
    RequestFailed(reqwest::Error),
    RequestError(String),
    DecodeResponseFailed(reqwest::Error),
    WsConnectFailed(String),
    WsRequestFailed(String),
    InvalidResponse(String),
    ChainNotSupported,
    NewBidError(String),
}

enum DecodedResponse<T: DeserializeOwned> {
    Ok(T),
    Err(ErrorBodyResponse),
}

impl<'de, T: DeserializeOwned> serde::Deserialize<'de> for DecodedResponse<T> {
    fn deserialize<D>(deserializer: D) -> Result<DecodedResponse<T>, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        let json_value = serde_json::Value::deserialize(deserializer)?;
        let value: Result<T, serde_json::Error> = serde_json::from_value(json_value.clone());
        match value {
            Ok(response) => Ok(DecodedResponse::Ok(response)),
            Err(error) => serde_json::from_value(json_value)
                .map(DecodedResponse::Err)
                .map_err(|_| serde::de::Error::custom(error)),
        }
    }
}

type WsRequest = (
    api_types::ws::ClientRequest,
    oneshot::Sender<ServerResultMessage>,
);

pub struct WsClientInner {
    #[allow(dead_code)]
    ws:             tokio::task::JoinHandle<()>,
    request_sender: mpsc::UnboundedSender<WsRequest>,
    request_id:     RwLock<u64>,

    update_receiver: broadcast::Receiver<api_types::ws::ServerUpdateResponse>,
}

#[derive(Clone)]
pub struct WsClient {
    inner: Arc<WsClientInner>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum MessageType {
    Response(api_types::ws::ServerResultResponse),
    Update(api_types::ws::ServerUpdateResponse),
}

pub struct WsClientUpdateStream<'a> {
    stream:    BroadcastStream<api_types::ws::ServerUpdateResponse>,
    _lifetime: PhantomData<&'a ()>,
}

impl WsClientUpdateStream<'_> {
    pub fn new(stream: BroadcastStream<api_types::ws::ServerUpdateResponse>) -> Self {
        Self {
            stream,
            _lifetime: PhantomData,
        }
    }
}

// Implementing Stream trait
impl Stream for WsClientUpdateStream<'_> {
    type Item = Result<api_types::ws::ServerUpdateResponse, BroadcastStreamRecvError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let stream = &mut self.get_mut().stream;
        stream.poll_next_unpin(cx)
    }
}

impl WsClient {
    pub fn get_update_stream(&self) -> WsClientUpdateStream {
        WsClientUpdateStream::new(BroadcastStream::new(
            self.inner.update_receiver.resubscribe(),
        ))
    }

    async fn run(
        mut ws_stream: WebSocketStream<MaybeTlsStream<TcpStream>>,
        mut request_receiver: mpsc::UnboundedReceiver<WsRequest>,
        update_sender: broadcast::Sender<api_types::ws::ServerUpdateResponse>,
    ) {
        let mut connection_check = tokio::time::interval(Duration::from_secs(1));
        let ping_duration = Duration::from_secs(32); // 30 seconds + 2 seconds to account for extra latency
        let mut latest_ping = tokio::time::Instant::now();
        let mut requests_map = HashMap::<String, oneshot::Sender<ServerResultMessage>>::new();
        loop {
            tokio::select! {
                message = ws_stream.next() => {
                    let message = match message {
                        Some(message) => {
                            match message {
                                Ok(message) => message,
                                Err(_) => continue,
                            }
                        }
                        None => break,
                    };

                    let message = match message {
                        Message::Text(text) => {
                            let response: Result<MessageType, serde_json::Error> = serde_json::from_str(&text);
                            match response {
                                Ok(response) => response,
                                Err(_) => continue,
                            }
                        }
                        Message::Binary(binary) => {
                            let response: Result<MessageType, serde_json::Error> = serde_json::from_slice(binary.as_slice());
                            match response {
                                Ok(response) => response,
                                Err(_) => continue,
                            }
                        }
                        Message::Close(_) => break,
                        Message::Pong(_) => continue,
                        Message::Ping(data) => {
                            latest_ping = tokio::time::Instant::now();
                            let _ = ws_stream.send(Message::Pong(data)).await;
                            continue;
                        },
                        Message::Frame(_) => continue,
                    };

                    match message {
                        MessageType::Response(response) => {
                            response.id.and_then(|id| requests_map.remove(&id)).map(|sender| sender.send(response.result));
                        }
                        MessageType::Update(update) => {
                            _ = update_sender.send(update);
                            continue;
                        }
                    }
                }
                request = request_receiver.recv() => {
                    match request {
                        Some((request, response_sender)) => {
                            if ws_stream.send(Message::Text(serde_json::to_string(&request).unwrap())).await.is_ok() {
                                requests_map.insert(request.id.clone(), response_sender);
                            }
                        }
                        None => break,
                    }
                }
                _  = connection_check.tick() => {
                    if latest_ping.elapsed() > ping_duration {
                        break;
                    }
                },
            }
        }
    }

    async fn fetch_add_request_id(&self) -> u64 {
        let mut write_gaurd = self.inner.request_id.write().await;
        *write_gaurd += 1;
        *write_gaurd
    }

    async fn send(
        &self,
        message: api_types::ws::ClientMessage,
    ) -> Result<ServerResultMessage, ClientError> {
        let request_id = self.fetch_add_request_id().await;
        let request = api_types::ws::ClientRequest {
            id:  request_id.to_string(),
            msg: message,
        };

        let (response_sender, response_receiver) = oneshot::channel();
        if self
            .inner
            .request_sender
            .send((request, response_sender))
            .is_err()
        {
            return Err(ClientError::WsRequestFailed(
                "Failed to send request".to_string(),
            ));
        }

        match tokio::time::timeout(Duration::from_secs(5), response_receiver).await {
            Ok(response) => match response {
                Ok(response) => Ok(response),
                Err(_) => Err(ClientError::WsRequestFailed(
                    "Response channel closed".to_string(),
                )),
            },
            // TODO: Clear this request from the requests_map
            Err(_) => Err(ClientError::WsRequestFailed(
                "Ws request timeout".to_string(),
            )),
        }
    }

    pub async fn chain_subscribe(&self, chain_ids: Vec<String>) -> Result<(), ClientError> {
        let message = api_types::ws::ClientMessage::Subscribe {
            chain_ids: chain_ids
                .iter()
                .map(|chain_id| chain_id.to_string())
                .collect(),
        };
        let result = self.send(message).await?;
        match result {
            ServerResultMessage::Success(_) => Ok(()),
            ServerResultMessage::Err(error) => Err(ClientError::WsRequestFailed(error)),
        }
    }

    pub async fn chain_unsubscribe(&self, chain_ids: Vec<String>) -> Result<(), ClientError> {
        let message = api_types::ws::ClientMessage::Unsubscribe {
            chain_ids: chain_ids
                .iter()
                .map(|chain_id| chain_id.to_string())
                .collect(),
        };
        let result = self.send(message).await?;
        match result {
            ServerResultMessage::Success(_) => Ok(()),
            ServerResultMessage::Err(error) => Err(ClientError::WsRequestFailed(error)),
        }
    }

    pub async fn submit_bid(
        &self,
        bid: api_types::bid::BidCreate,
    ) -> Result<api_types::bid::BidResult, ClientError> {
        let message = api_types::ws::ClientMessage::PostBid { bid };
        let result = self.send(message).await?;
        match result {
            ServerResultMessage::Success(response) => {
                let response = response.ok_or(ClientError::InvalidResponse(
                    "Invalid server response: Expected BidResult but got None.".to_string(),
                ))?;
                let api_types::ws::APIResponse::BidResult(response) = response;
                Ok(response)
            }
            ServerResultMessage::Err(error) => Err(ClientError::WsRequestFailed(error)),
        }
    }
}

impl Client {
    async fn decode<T: DeserializeOwned>(response: Response) -> Result<T, ClientError> {
        match response.json().await {
            Ok(DecodedResponse::Ok(response)) => Ok(response),
            Ok(DecodedResponse::Err(response)) => Err(ClientError::RequestError(response.error)),
            Err(e) => Err(ClientError::DecodeResponseFailed(e)),
        }
    }

    async fn send<T: Serialize, R: DeserializeOwned>(
        &self,
        route: impl Routable,
        query: Option<T>,
    ) -> Result<R, ClientError> {
        let properties = route.properties();
        let url = self
            .http_url
            .join(properties.full_path.as_str())
            .map_err(|e| ClientError::InvalidHttpUrl(e.to_string()))?;
        let mut request = self.client.request(properties.method, url);
        if let Some(api_key) = self.api_key.clone() {
            request = request.bearer_auth(api_key);
        }
        if let Some(query) = query {
            request = request.query(&query);
        }
        let response = request.send().await.map_err(ClientError::RequestFailed)?;
        Client::decode(response).await
    }

    pub fn try_new(config: ClientConfig) -> Result<Self, ClientError> {
        let http_url = Url::parse(config.http_url.as_str())
            .map_err(|e| ClientError::InvalidHttpUrl(e.to_string()))?;

        if http_url.scheme() != "http" && http_url.scheme() != "https" {
            return Err(ClientError::InvalidHttpUrl(format!(
                "Invalid scheme {}",
                http_url.scheme()
            )));
        }

        let ws_url_string = if http_url.scheme() == "http" {
            config.http_url.replace("http", "ws")
        } else {
            config.http_url.replace("https", "wss")
        };
        let ws_url = Url::parse(ws_url_string.as_str()).expect("Failed to parse ws url");

        Ok(Self {
            http_url,
            ws_url,
            api_key: config.api_key,
            client: reqwest::Client::new(),
        })
    }

    pub async fn connect_websocket(&self) -> Result<WsClient, ClientError> {
        let url = self
            .ws_url
            .join(api_types::ws::Route::Ws.properties().full_path.as_str())
            .map_err(|e| ClientError::WsConnectFailed(e.to_string()))?;
        let mut request = url
            .as_str()
            .into_client_request()
            .map_err(|e| ClientError::WsConnectFailed(e.to_string()))?;
        if let Some(api_key) = self.api_key.clone() {
            let bearer_token = format!("Bearer {}", api_key);
            request.headers_mut().insert(
                "Authorization",
                bearer_token.parse().map_err(|_| {
                    ClientError::WsConnectFailed("Failed to parse api key".to_string())
                })?,
            );
        }
        let (ws_stream, _) = connect_async(request)
            .await
            .map_err(|e| ClientError::WsConnectFailed(e.to_string()))?;

        let (request_sender, request_receiver) = mpsc::unbounded_channel();
        let (update_sender, update_receiver) = broadcast::channel(1000);

        Ok(WsClient {
            inner: Arc::new(WsClientInner {
                request_sender,
                update_receiver,
                request_id: RwLock::new(0),
                ws: tokio::spawn(WsClient::run(ws_stream, request_receiver, update_sender)),
            }),
        })
    }

    pub async fn get_opportunities(
        &self,
        params: Option<GetOpportunitiesQueryParams>,
    ) -> Result<Vec<Opportunity>, ClientError> {
        self.send(Route::GetOpportunities, params).await
    }

    pub async fn new_bid<T: Biddable>(
        opportunity: T,
        params: T::Params,
        private_key: String,
    ) -> Result<api_types::bid::BidCreate, ClientError> {
        T::new_bid(opportunity, params, private_key)
    }
}

pub trait Biddable {
    type Params;

    fn new_bid(
        opportunity: Self,
        params: Self::Params,
        private_key: String,
    ) -> Result<api_types::bid::BidCreate, ClientError>;
}

impl Biddable for api_types::opportunity::OpportunityEvm {
    type Params = BidParamsEvm;

    fn new_bid(
        opportunity: Self,
        bid_params: Self::Params,
        private_key: String,
    ) -> Result<BidCreate, ClientError> {
        let private_key = private_key.parse::<LocalWallet>().map_err(|e| {
            ClientError::NewBidError(format!("Failed to parse private key: {:?}", e))
        })?;
        let params = get_params(opportunity.clone());
        let config = get_config(params.chain_id.as_str())?;
        let wallet = LocalWallet::from(private_key);
        let bid = BidCreateEvm {
            permission_key:  params.permission_key,
            chain_id:        params.chain_id,
            target_contract: config.adapter_factory_contract,
            amount:          bid_params.amount,
            target_calldata: make_adapter_calldata(opportunity.clone(), bid_params, wallet)?,
        };
        Ok(BidCreate::Evm(bid))
    }
}

impl Biddable for api_types::opportunity::OpportunitySvm {
    type Params = i64;

    fn new_bid(
        _opportunity: Self,
        _params: Self::Params,
        _private_key: String,
    ) -> Result<BidCreate, ClientError> {
        todo!()
    }
}
