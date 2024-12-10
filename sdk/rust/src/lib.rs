pub use {
    ethers,
    express_relay_api_types as api_types,
};
use {
    evm::BidParamsEvm,
    express_relay_api_types::{
        opportunity::{
            GetOpportunitiesQueryParams,
            Opportunity,
            Route,
        },
        ws::ServerResultMessage,
        AccessLevel,
        ErrorBodyResponse,
        RouteTrait,
    },
    futures_util::{
        SinkExt,
        StreamExt,
    },
    reqwest::Response,
    serde::{
        de::DeserializeOwned,
        Deserialize,
        Serialize,
    },
    std::{
        collections::BTreeMap,
        time::Duration,
    },
    strum::{
        Display,
        EnumString,
    },
    tokio::{
        net::TcpStream,
        sync::{
            mpsc,
            oneshot,
            RwLock,
        },
        time::sleep,
    },
    tokio_stream::wrappers::UnboundedReceiverStream,
    tokio_tungstenite::{
        connect_async,
        tungstenite::Message,
        MaybeTlsStream,
        WebSocketStream,
    },
    url::Url,
};

pub mod evm;

#[derive(Display, EnumString)]
pub enum ChainId {
    #[strum(serialize = "development")]
    DevelopmentEvm,
    #[strum(serialize = "development-solana")]
    DevelopmentSvm,
    #[strum(serialize = "op-sepolia")]
    OpSepolia,
    #[strum(serialize = "solana")]
    Solana,
    #[strum(serialize = "mode")]
    Mode,
}

pub struct Client {
    http_url: Url,
    ws_url:   Url,
    api_key:  Option<String>,
    client:   reqwest::Client,
}

pub struct ClientConfig {
    pub http_url: String,
    pub ws_url:   String,
    pub api_key:  Option<String>,
}

#[derive(Debug)]
pub enum ClientError {
    InvalidHttpUrl(String),
    InvalidWsUrl(String),
    RequestFailed(reqwest::Error),
    RequestError(String),
    DecodeResponseFailed(reqwest::Error),
    AuthenticationRequired,
    SubscribeFailed(String),
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

pub struct WsClient {
    #[allow(dead_code)]
    ws:             tokio::task::JoinHandle<()>,
    request_sender: mpsc::UnboundedSender<WsRequest>,
    request_id:     RwLock<usize>,

    pub update_stream: RwLock<UnboundedReceiverStream<api_types::ws::ServerUpdateResponse>>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum MessageType {
    Response(api_types::ws::ServerResultResponse),
    Update(api_types::ws::ServerUpdateResponse),
}

impl WsClient {
    async fn run(
        mut ws_stream: WebSocketStream<MaybeTlsStream<TcpStream>>,
        mut request_receiver: mpsc::UnboundedReceiver<WsRequest>,
        update_sender: mpsc::UnboundedSender<api_types::ws::ServerUpdateResponse>,
    ) {
        let mut requests_map = BTreeMap::<String, oneshot::Sender<ServerResultMessage>>::new();
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
                            if update_sender.send(update).is_err() {
                                break;
                            }
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
            }
        }
    }

    async fn send(
        &self,
        message: api_types::ws::ClientMessage,
    ) -> Result<ServerResultMessage, ClientError> {
        let mut write_gaurd = self.request_id.write().await;
        let request_id = write_gaurd.to_string();
        *write_gaurd += 1;
        drop(write_gaurd);

        let request = api_types::ws::ClientRequest {
            id:  request_id.clone(),
            msg: message,
        };

        let (response_sender, response_receiver) = oneshot::channel();
        if self
            .request_sender
            .send((request, response_sender))
            .is_err()
        {
            return Err(ClientError::SubscribeFailed(
                "Failed to send request".to_string(),
            ));
        }

        match tokio::time::timeout(Duration::from_secs(5), response_receiver).await {
            Ok(response) => match response {
                Ok(response) => Ok(response),
                Err(_) => Err(ClientError::SubscribeFailed(
                    "Response channel closed".to_string(),
                )),
            },
            Err(_) => Err(ClientError::SubscribeFailed(
                "Ws request timeout".to_string(),
            )),
        }
    }

    pub async fn chain_subscribe(&self, chain_ids: Vec<ChainId>) -> Result<(), ClientError> {
        let message = api_types::ws::ClientMessage::Subscribe {
            chain_ids: chain_ids
                .iter()
                .map(|chain_id| chain_id.to_string())
                .collect(),
        };
        let result = self.send(message).await?;
        match result {
            ServerResultMessage::Success(_) => Ok(()),
            ServerResultMessage::Err(error) => Err(ClientError::SubscribeFailed(error)),
        }
    }

    pub async fn chain_unsubscribe(&self, chain_ids: Vec<ChainId>) -> Result<(), ClientError> {
        let message = api_types::ws::ClientMessage::Unsubscribe {
            chain_ids: chain_ids
                .iter()
                .map(|chain_id| chain_id.to_string())
                .collect(),
        };
        let result = self.send(message).await?;
        match result {
            ServerResultMessage::Success(_) => Ok(()),
            ServerResultMessage::Err(error) => Err(ClientError::SubscribeFailed(error)),
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
                let response =
                    response.ok_or(ClientError::InvalidResponse("Invalid response".to_string()))?;
                let api_types::ws::APIResponse::BidResult(response) = response;
                Ok(response)
            }
            ServerResultMessage::Err(error) => Err(ClientError::SubscribeFailed(error)),
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
        route: impl RouteTrait,
        query: Option<T>,
    ) -> Result<R, ClientError> {
        if self.api_key.is_none() && route.access_level() != AccessLevel::Public {
            return Err(ClientError::AuthenticationRequired);
        }

        let url = self
            .http_url
            .join(route.full_path().as_str())
            .map_err(|e| ClientError::InvalidHttpUrl(e.to_string()))?;
        let mut request = self.client.request(route.method(), url);
        if let Some(query) = query {
            request = request.query(&query);
        }
        let response = request.send().await.map_err(ClientError::RequestFailed)?;
        Client::decode(response).await
    }

    pub fn try_new(config: ClientConfig) -> Result<Self, ClientError> {
        let http_url = Url::parse(config.http_url.as_str())
            .map_err(|e| ClientError::InvalidHttpUrl(e.to_string()))?;
        let ws_url = Url::parse(config.ws_url.as_str())
            .map_err(|e| ClientError::InvalidWsUrl(e.to_string()))?;

        if http_url.scheme() != "http" && http_url.scheme() != "https" {
            return Err(ClientError::InvalidHttpUrl("Invalid scheme".to_string()));
        }

        if ws_url.scheme() != "ws" && ws_url.scheme() != "wss" {
            return Err(ClientError::InvalidWsUrl("Invalid scheme".to_string()));
        }

        Ok(Self {
            http_url,
            ws_url,
            api_key: config.api_key,
            client: reqwest::Client::new(),
        })
    }

    pub async fn connect_websocket(&self) -> Result<WsClient, ClientError> {
        let url_string = format!(
            "{}{}{}",
            self.ws_url.as_str().trim_end_matches("/"),
            api_types::Route::V1.as_ref(),
            api_types::Route::Ws.as_ref()
        );
        let (ws_stream, _) = connect_async(url_string)
            .await
            .map_err(|e| ClientError::SubscribeFailed(e.to_string()))?;

        let (request_sender, request_receiver) = mpsc::unbounded_channel();
        let (update_sender, update_receiver) = mpsc::unbounded_channel();

        Ok(WsClient {
            request_sender,
            update_stream: RwLock::new(UnboundedReceiverStream::<
                api_types::ws::ServerUpdateResponse,
            >::new(update_receiver)),
            request_id: RwLock::new(0),
            ws: tokio::spawn(WsClient::run(ws_stream, request_receiver, update_sender)),
        })
    }

    pub async fn get_opportunities(
        &self,
        params: Option<GetOpportunitiesQueryParams>,
    ) -> Result<Vec<Opportunity>, ClientError> {
        self.send(Route::GetOpportunities, params).await
    }

    pub async fn new_bid<T: OpportunityTrait>(
        _opportunity: T,
        _params: T::Params,
    ) -> Result<api_types::bid::BidCreate, ClientError> {
        sleep(Duration::from_secs(5)).await;
        Err(ClientError::ChainNotSupported)
    }
}

pub trait OpportunityTrait {
    type Params;
    fn new_bid(
        opportunity: Self,
        params: Self::Params,
    ) -> Result<api_types::bid::BidCreate, ClientError>;
}

impl OpportunityTrait for api_types::opportunity::OpportunityEvm {
    type Params = BidParamsEvm;

    fn new_bid(
        _opportunity: Self,
        _params: Self::Params,
    ) -> Result<express_relay_api_types::bid::BidCreate, ClientError> {
        todo!()
    }
}

impl OpportunityTrait for api_types::opportunity::OpportunitySvm {
    type Params = i64;

    fn new_bid(
        _opportunity: Self,
        _params: Self::Params,
    ) -> Result<express_relay_api_types::bid::BidCreate, ClientError> {
        todo!()
    }
}
