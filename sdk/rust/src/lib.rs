pub use express_relay_api_types as api_types;
use {
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
        Serialize,
    },
    std::{
        collections::BTreeMap,
        time::Duration,
    },
    strum::Display,
    tokio::{
        net::TcpStream,
        sync::{
            mpsc,
            oneshot,
        },
    },
    tokio_tungstenite::{
        connect_async,
        tungstenite::Message,
        MaybeTlsStream,
        WebSocketStream,
    },
    url::Url,
};

#[derive(Display)]
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
    request_id:     usize,
}

impl WsClient {
    async fn run(
        mut ws_stream: WebSocketStream<MaybeTlsStream<TcpStream>>,
        mut request_receiver: mpsc::UnboundedReceiver<WsRequest>,
    ) {
        let mut requests_map = BTreeMap::<String, oneshot::Sender<ServerResultMessage>>::new();
        loop {
            tokio::select! {
                message = ws_stream.next() => {
                    match message {
                        Some(message) => {
                            match message {
                                Ok(Message::Text(text)) => {
                                    let response: Result<api_types::ws::ServerResultResponse, serde_json::Error> = serde_json::from_str(&text);
                                    match response {
                                        Ok(response) => {
                                            if let Some(id) = response.id.clone() {
                                                if let Some(sender) = requests_map.remove(&id) {
                                                    if sender.send(response.result).is_err() {
                                                        eprintln!("Failed to send response for {}", id);
                                                    }
                                                }
                                            }
                                        },
                                        Err(e) => {
                                            eprintln!("Failed to decode response: {} - {}", e, text);
                                        },
                                    }
                                },
                                Ok(Message::Close(_)) => break,
                                _ => (),
                            }
                        }
                        None => {
                            eprint!("Ws stream closed");
                            break;
                        }
                    }
                }
                request = request_receiver.recv() => {
                    match request {
                        Some((request, response_sender)) => {
                            match ws_stream.send(Message::Text(serde_json::to_string(&request).unwrap())).await {
                                Ok(_) => {
                                    requests_map.insert(request.id.clone(), response_sender);
                                },
                                Err(e) => eprintln!("Failed to send request: {}", e),
                            }
                        }
                        None => {
                            eprint!("Request sender closed");
                            break;
                        }
                    }
                }
            }
        }
    }

    pub async fn chain_subscribe(&mut self, chain_ids: Vec<ChainId>) -> Result<(), ClientError> {
        let message = api_types::ws::ClientMessage::Subscribe {
            chain_ids: chain_ids
                .iter()
                .map(|chain_id| chain_id.to_string())
                .collect(),
        };
        let request_id = self.request_id.to_string();
        self.request_id += 1;
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
                Ok(response) => match response {
                    api_types::ws::ServerResultMessage::Success(_) => Ok(()),
                    api_types::ws::ServerResultMessage::Err(error) => {
                        Err(ClientError::SubscribeFailed(error))
                    }
                },
                Err(_) => Err(ClientError::SubscribeFailed(
                    "Response channel closed".to_string(),
                )),
            },
            Err(_) => Err(ClientError::SubscribeFailed(
                "Ws request timeout".to_string(),
            )),
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

        Ok(WsClient {
            request_sender,
            request_id: 0,
            ws: tokio::spawn(WsClient::run(ws_stream, request_receiver)),
        })
    }

    pub async fn get_opportunities(
        &self,
        params: Option<GetOpportunitiesQueryParams>,
    ) -> Result<Vec<Opportunity>, ClientError> {
        self.send(Route::GetOpportunities, params).await
    }
}
