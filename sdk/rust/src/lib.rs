pub use {
    ethers,
    express_relay_api_types as api_types,
    solana_sdk,
};
use {
    express_relay_api_types::{
        bid::{
            BidCreate,
            BidCreateEvm,
            BidCreateOnChainSvm,
            BidCreateSvm,
            BidCreateSwapSvm,
            BidCreateSwapSvmTag,
        },
        opportunity::{
            FeeToken,
            GetOpportunitiesQueryParams,
            Opportunity,
            OpportunityParamsSvm,
            OpportunityParamsV1ProgramSvm,
            QuoteTokens,
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
    solana_sdk::transaction::Transaction,
    spl_token::native_mint,
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
    svm::{
        GetSubmitBidInstructionParams,
        GetSwapInstructionParams,
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
pub mod svm;

pub struct ClientInner {
    http_url: Url,
    ws_url:   Url,
    api_key:  Option<String>,
    client:   reqwest::Client,
    evm:      evm::Evm,
}

#[derive(Clone)]
pub struct Client {
    inner: Arc<ClientInner>,
}

#[derive(Debug, Clone)]
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
    SvmError(String),
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

/// A stream of WebSocket updates received from the server.
///
/// # Developer Notes
///
/// - This struct wraps a `BroadcastStream` that delivers updates as `ServerUpdateResponse` objects.
/// - The `PhantomData` ensures that the lifetime of this stream is explicitly tied to the `WsClient` instance.
///
/// ## Why PhantomData?
///
/// - `PhantomData<&'a ()>` acts as a marker to indicate that this struct's lifetime `'a`
///   depends on the `WsClient` that created it.
/// - Without `PhantomData`, the compiler cannot ensure that the `WsClientUpdateStream` does not outlive
///   the `WsClient`. This can lead to dangling references or invalid state.
/// - By including `PhantomData`, the borrow checker guarantees at compile time that the stream
///   remains valid only as long as the `WsClient` exists.
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
    /// Retrieves a stream of WebSocket updates from the server.
    ///
    /// # Returns
    ///
    /// * `WsClientUpdateStream` - A stream of updates that can be polled asynchronously.
    ///
    /// # Lifetime
    ///
    /// The lifetime of the update stream is guaranteed at compile time to be tied to the `WsClient`.
    /// If the `WsClient` is dropped, the stream will also become invalid.
    pub fn get_update_stream(&self) -> WsClientUpdateStream {
        WsClientUpdateStream::new(BroadcastStream::new(
            self.inner.update_receiver.resubscribe(),
        ))
    }

    /// Runs the WebSocket event loop, managing incoming messages, outgoing requests, and connection health.
    ///
    /// # Developer Notes
    ///
    /// - This function runs continuously and listens for three main events:
    ///   1. **Incoming WebSocket messages**: Handles text, binary, ping, and close frames.
    ///      - WebSocket messages can be of two types:
    ///         - **Updates**: Broadcasted to all clients via the `update_sender` channel.
    ///         - **Responses**: Sent as a response to a specific client request and delivered to the
    ///           corresponding `oneshot` channel for that request (tracked via `requests_map`).
    ///   2. **Requests from the client**: Sends messages through the WebSocket when received from the request channel.
    ///   3. **Connection health check**: Monitors for pings to ensure the connection is alive.
    ///
    /// - Uses a `HashMap` (`requests_map`) to track pending requests and match responses based on their IDs.
    /// - If no ping is received for 32 seconds, the function assumes the connection is broken and terminates.
    ///
    /// This function is spawned as a background task and must be resilient to message errors
    /// or other intermittent failures.
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
        let mut write_guard = self.inner.request_id.write().await;
        *write_guard += 1;
        *write_guard
    }

    /// Sends a WebSocket message and waits for a response.
    ///
    /// # Developer Notes
    ///
    /// - Generates a unique request ID using `fetch_add_request_id` to match requests with responses.
    /// - Sends a `ClientRequest` message through the internal `request_sender` channel.
    /// - Uses a `oneshot` channel to wait for the response corresponding to the request ID.
    /// - Times out after 5 seconds if no response is received, returning a `WsRequestFailed` error.
    ///
    /// **Request Matching**:
    /// Responses are matched to their corresponding requests via the `requests_map` in the `run` loop.
    /// If the timeout occurs, developers must ensure that orphaned requests are handled appropriately.
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

    /// Subscribes to updates for specific blockchains.
    ///
    /// # Arguments
    ///
    /// * `chain_ids` - A vector of chain IDs as strings.
    ///
    /// # Returns
    ///
    /// * `Result<(), ClientError>` - Returns `Ok(())` on success or an error.
    ///
    /// # Errors
    ///
    /// Returns an error if the subscription request fails or times out.
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

    /// Unsubscribes from updates for specific blockchains.
    ///
    /// # Arguments
    ///
    /// * `chain_ids` - A vector of chain IDs as strings.
    ///
    /// # Returns
    ///
    /// * `Result<(), ClientError>` - Returns `Ok(())` on success or an error.
    ///
    /// # Errors
    ///
    /// Returns an error if the unsubscription request fails or times out.
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

    /// Submits a bid to the server.
    ///
    /// # Arguments
    ///
    /// * `bid` - The bid object to be submitted, which contains the relevant parameters for the transaction.
    ///
    /// # Returns
    ///
    /// * `Result<api_types::bid::BidResult, ClientError>` - The result of the bid submission.
    ///
    /// # Errors
    ///
    /// Returns an error if the WebSocket request fails or the server responds with an error.
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

    /// Cancel a bid.
    ///
    /// # Arguments
    ///
    /// * `bid_cancel` - The data needed to cancel bid.
    ///
    /// # Returns
    ///
    /// * `Result<(), ClientError>` - The result of the bid cancellation.
    ///
    /// # Errors
    ///
    /// Returns an error if the WebSocket request fails or the server responds with an error.
    pub async fn cancel_bid(
        &self,
        bid_cancel: api_types::bid::BidCancel,
    ) -> Result<(), ClientError> {
        let message = api_types::ws::ClientMessage::CancelBid { data: bid_cancel };
        let result = self.send(message).await?;
        match result {
            ServerResultMessage::Success(_) => Ok(()),
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

    /// Sends an HTTP request to the server and decodes the response.
    ///
    /// # Developer Notes
    ///
    /// - Constructs an HTTP request using the specified route and optional query parameters.
    /// - If an `api_key` is set, it adds a `Bearer` authorization header to the request.
    /// - This function expects the server response to conform to the following structure:
    ///    - `DecodedResponse::Ok` for successful responses.
    ///    - `DecodedResponse::Err` for error bodies returned by the server.
    /// - The function uses `reqwest::Client` internally and decodes the response using `serde`.
    ///
    /// # Parameters
    ///
    /// - `route` - Defines the API endpoint and HTTP method via the `Routable` trait.
    /// - `query` - Optional query parameters that are serialized into the request URL.
    ///
    /// # Implementation Details
    ///
    /// - If the HTTP response is valid but contains an error body that can be decoded to `ErrorBodyResponse`, the function returns a
    ///   `ClientError::RequestError` with the server's error message.
    /// - If the HTTP response fails to decode, it returns `ClientError::DecodeResponseFailed`.
    /// - Errors due to request failure (e.g., network issues) are returned as `ClientError::RequestFailed`.
    ///
    /// **Timeouts**:
    /// The default `reqwest` client timeout applies here. Ensure proper timeout handling in the caller.
    async fn send<T: Serialize, R: DeserializeOwned>(
        &self,
        route: impl Routable,
        query: Option<T>,
    ) -> Result<R, ClientError> {
        // TODO add params and body here
        let properties = route.properties();
        let url = self
            .inner
            .http_url
            .join(properties.full_path.as_str())
            .map_err(|e| ClientError::InvalidHttpUrl(e.to_string()))?;
        let mut request = self.inner.client.request(properties.method, url);
        if let Some(api_key) = self.inner.api_key.clone() {
            request = request.bearer_auth(api_key);
        }
        if let Some(query) = query {
            request = request.query(&query);
        }
        let response = request.send().await.map_err(ClientError::RequestFailed)?;
        Client::decode(response).await
    }

    fn get_urls(config: ClientConfig) -> Result<(Url, Url), ClientError> {
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

        Ok((http_url, ws_url))
    }

    /// Creates a new HTTP client with the provided configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - The client configuration containing an HTTP URL and an optional API key.
    ///
    /// # Returns
    ///
    /// * `Result<Self, ClientError>` - A result containing the initialized client or an error.
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP URL is invalid or has an unsupported scheme.
    pub fn try_new(config: ClientConfig) -> Result<Self, ClientError> {
        let (http_url, ws_url) = Self::get_urls(config.clone())?;
        Ok(Self {
            inner: Arc::new(ClientInner {
                http_url,
                ws_url,
                api_key: config.api_key,
                client: reqwest::Client::new(),
                evm: evm::Evm::new(None),
            }),
        })
    }

    /// Creates a new HTTP client with the provided configuration and EVM configuration.
    /// This is for developers who want to use a custom EVM configuration.
    /// Do not use this method unless you are sure about the configuration.
    pub fn try_new_with_evm_config(
        config: ClientConfig,
        evm_config: HashMap<String, evm::Config>,
    ) -> Result<Self, ClientError> {
        let (http_url, ws_url) = Self::get_urls(config.clone())?;
        Ok(Self {
            inner: Arc::new(ClientInner {
                http_url,
                ws_url,
                api_key: config.api_key,
                client: reqwest::Client::new(),
                evm: evm::Evm::new(Some(evm_config)),
            }),
        })
    }

    /// Establishes a WebSocket connection to the server.
    ///
    /// # Returns
    ///
    /// * `Result<WsClient, ClientError>` - A thread-safe WebSocket client for interacting with the server.
    ///
    /// # Errors
    ///
    /// Returns an error if the connection or the WebSocket handshake fails.
    ///
    /// # Thread Safety
    ///
    /// The returned `WsClient` is thread-safe and can be cloned to share across multiple tasks.
    pub async fn connect_websocket(&self) -> Result<WsClient, ClientError> {
        let url = self
            .inner
            .ws_url
            .join(api_types::ws::Route::Ws.properties().full_path.as_str())
            .map_err(|e| ClientError::WsConnectFailed(e.to_string()))?;
        let mut request = url
            .as_str()
            .into_client_request()
            .map_err(|e| ClientError::WsConnectFailed(e.to_string()))?;
        if let Some(api_key) = self.inner.api_key.clone() {
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

    /// Fetches opportunities based on optional query parameters.
    ///
    /// # Arguments
    ///
    /// * `params` - Optional query parameters for filtering opportunities.
    ///
    /// # Returns
    ///
    /// * `Result<Vec<Opportunity>, ClientError>` - A list of opportunities or an error.
    pub async fn get_opportunities(
        &self,
        params: Option<GetOpportunitiesQueryParams>,
    ) -> Result<Vec<Opportunity>, ClientError> {
        self.send(Route::GetOpportunities, params).await
    }

    /// Creates a new bid for an opportunity.
    ///
    /// # Type Parameters
    ///
    /// * `T` - A type that implements the `Biddable` trait.
    ///
    /// # Arguments
    ///
    /// * `opportunity` - The opportunity to bid on.
    /// * `params` - Bid parameters specific to the opportunity type.
    ///
    /// # Returns
    ///
    /// * `Result<BidCreate, ClientError>` - A bid creation object or an error.
    pub async fn new_bid<T: Biddable>(
        &self,
        opportunity: T,
        params: T::Params,
    ) -> Result<api_types::bid::BidCreate, ClientError> {
        T::new_bid(self, opportunity, params)
    }
}

pub trait Biddable {
    type Params;

    fn new_bid(
        client: &Client,
        opportunity: Self,
        params: Self::Params,
    ) -> Result<BidCreate, ClientError>;
}

impl Biddable for api_types::opportunity::OpportunityEvm {
    type Params = evm::NewBidParams;

    fn new_bid(
        client: &Client,
        opportunity: Self,
        params: Self::Params,
    ) -> Result<BidCreate, ClientError> {
        let opportunity_params = evm::get_params(opportunity.clone());
        let config = client
            .inner
            .evm
            .get_config(opportunity_params.chain_id.as_str())?;
        let bid = BidCreateEvm {
            permission_key:  opportunity_params.permission_key,
            chain_id:        opportunity_params.chain_id,
            target_contract: config.adapter_factory_contract,
            amount:          params.bid_params.amount,
            target_calldata: client.inner.evm.make_adapter_calldata(
                opportunity.clone(),
                params.bid_params,
                params.wallet,
            )?,
        };
        Ok(BidCreate::Evm(bid))
    }
}

impl Biddable for api_types::opportunity::OpportunitySvm {
    type Params = svm::NewBidParams;

    /// Creates a new bid for an SVM opportunity.
    ///
    /// It receives a list of instructions and add the "submit_bid" or "swap" instruction to it based on the opportunity type.
    /// Then it creates a transaction with the instructions and partially signs it with the signers.
    /// Finally, it returns a Bid object with the created transaction.
    /// If you don't want to use this method, you can use the svm::Svm::get_submit_bid_instruction or svm::Svm::get_swap_instruction methods to create the "submit_bid" or "swap" instruction and manually create the transaction and bid object.
    fn new_bid(
        _client: &Client,
        opportunity: Self,
        params: Self::Params,
    ) -> Result<BidCreate, ClientError> {
        let OpportunityParamsSvm::V1(opportunity_params) = opportunity.params.clone();
        match opportunity_params.program {
            OpportunityParamsV1ProgramSvm::Limo { .. } => {
                let program_params = match params.program_params {
                    svm::ProgramParams::Limo(params) => Ok(params),
                    _ => Err(ClientError::NewBidError(
                        "Invalid program params for Limo opportunity".to_string(),
                    )),
                }?;
                let mut instructions = params.instructions;
                instructions.push(svm::Svm::get_submit_bid_instruction(
                    GetSubmitBidInstructionParams {
                        chain_id:             opportunity_params.chain_id.clone(),
                        amount:               params.amount,
                        deadline:             params.deadline,
                        searcher:             params.searcher,
                        permission:           program_params.permission,
                        router:               program_params.router,
                        relayer_signer:       params.relayer_signer,
                        fee_receiver_relayer: params.fee_receiver_relayer,
                    },
                )?);
                let mut transaction =
                    Transaction::new_with_payer(instructions.as_slice(), Some(&params.payer));
                transaction
                    .try_partial_sign(&params.signers, params.block_hash)
                    .map_err(|e| {
                        ClientError::NewBidError(format!("Failed to sign transaction: {:?}", e))
                    })?;
                Ok(BidCreate::Svm(BidCreateSvm::OnChain(BidCreateOnChainSvm {
                    chain_id:    opportunity_params.chain_id.clone(),
                    transaction: transaction.into(),
                    slot:        params.slot,
                })))
            }
            OpportunityParamsV1ProgramSvm::Swap {
                user_wallet_address,
                tokens,
                fee_token,
                router_account,
                referral_fee_bps,
                token_account_initialization_configs,
                ..
            } => {
                let _ = match params.program_params {
                    svm::ProgramParams::Swap(params) => Ok(params),
                    _ => Err(ClientError::NewBidError(
                        "Invalid program params for swap opportunity".to_string(),
                    )),
                }?;

                let (searcher_token, user_token, user_amount_including_fees) = match tokens.tokens {
                    QuoteTokens::SearcherTokenSpecified {
                        searcher_token,
                        user_token,
                        ..
                    } => (
                        searcher_token,
                        user_token,
                        svm::Svm::get_bid_amount_including_fees(
                            &opportunity.params,
                            params.amount,
                        )?,
                    ),
                    QuoteTokens::UserTokenSpecified {
                        searcher_token,
                        user_token,
                        user_amount_including_fees,
                        ..
                    } => (searcher_token, user_token, user_amount_including_fees),
                };
                let (fee_token, fee_token_program) = match fee_token {
                    FeeToken::SearcherToken => (searcher_token, tokens.token_program_searcher),
                    FeeToken::UserToken => (user_token, tokens.token_program_user),
                };
                let mut instructions = params.instructions;
                instructions.extend(svm::Svm::get_swap_create_accounts_idempotent_instructions(
                    svm::GetSwapCreateAccountsIdempotentInstructionsParams {
                        searcher: params.searcher,
                        user: user_wallet_address,
                        searcher_token,
                        token_program_searcher: tokens.token_program_searcher,
                        fee_token,
                        fee_token_program,
                        router_account,
                        fee_receiver_relayer: params.fee_receiver_relayer,
                        referral_fee_bps,
                        chain_id: opportunity_params.chain_id.clone(),
                        configs: token_account_initialization_configs,
                    },
                ));
                if user_token == native_mint::id() {
                    instructions.extend(svm::Svm::get_wrap_sol_instructions(
                        svm::GetWrapSolInstructionsParams {
                            payer:      params.payer,
                            owner:      user_wallet_address,
                            amount:     user_amount_including_fees,
                            create_ata: false,
                        },
                    )?);
                }
                instructions.push(svm::Svm::get_swap_instruction(GetSwapInstructionParams {
                    opportunity_params:   opportunity.params,
                    bid_amount:           params.amount,
                    deadline:             params.deadline,
                    searcher:             params.searcher,
                    fee_receiver_relayer: params.fee_receiver_relayer,
                    relayer_signer:       params.relayer_signer,
                })?);
                if searcher_token == native_mint::id() {
                    instructions.push(svm::Svm::get_unwrap_sol_instruction(
                        svm::GetUnwrapSolInstructionParams {
                            owner: user_wallet_address,
                        },
                    )?)
                }
                let mut transaction =
                    Transaction::new_with_payer(instructions.as_slice(), Some(&params.payer));
                transaction
                    .try_partial_sign(&params.signers, params.block_hash)
                    .map_err(|e| {
                        ClientError::NewBidError(format!("Failed to sign transaction: {:?}", e))
                    })?;
                Ok(BidCreate::Svm(BidCreateSvm::Swap(BidCreateSwapSvm {
                    chain_id:       opportunity_params.chain_id,
                    transaction:    transaction.into(),
                    opportunity_id: opportunity.opportunity_id,
                    _type:          BidCreateSwapSvmTag::Swap,
                })))
            }
        }
    }
}
