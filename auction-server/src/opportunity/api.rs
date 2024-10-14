use {
    super::{
        repository,
        service::{
            add_opportunity::AddOpportunityInput,
            estimate_price::EstimatePriceInput,
            get_opportunities::GetOpportunitiesInput,
            handle_opportunity_bid::HandleOpportunityBidInput,
        },
    },
    crate::{
        api::{
            Auth,
            ErrorBodyResponse,
            RestError,
        },
        kernel::entities::{
            ChainId,
            PermissionKey,
        },
        models,
        state::{
            BidId,
            StoreNew,
            UnixTimestampMicros,
        },
    },
    axum::{
        extract::{
            Path,
            Query,
            State,
        },
        routing::{
            get,
            post,
        },
        Json,
        Router,
    },
    ethers::types::{
        Address,
        Bytes,
        Signature,
        U256,
    },
    serde::{
        Deserialize,
        Serialize,
    },
    serde_with::{
        base64::Base64,
        serde_as,
        DisplayFromStr,
    },
    solana_sdk::{
        clock::Slot,
        hash::Hash,
        pubkey::Pubkey,
    },
    std::sync::Arc,
    time::OffsetDateTime,
    utoipa::{
        schema,
        IntoParams,
        ToResponse,
        ToSchema,
    },
    uuid::Uuid,
};

// Base types
pub type OpportunityId = Uuid;

#[derive(Debug, Serialize, Deserialize, ToSchema, Clone)]
#[serde(rename_all = "lowercase")]
pub enum OpportunityMode {
    Live,
    Historical,
}

#[derive(Serialize, Deserialize, ToResponse, ToSchema, Clone)]
pub struct OpportunityBidResult {
    #[schema(example = "OK")]
    pub status: String,
    /// The unique id created to identify the bid. This id can be used to query the status of the bid.
    #[schema(example = "beedbeed-58cc-4372-a567-0e02b2c3d479", value_type=String)]
    pub id:     BidId,
}

/// The input type for creating a new opportunity
#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug)]
#[serde(untagged)]
pub enum OpportunityCreate {
    #[schema(title = "evm")]
    Evm(OpportunityCreateEvm),
    #[schema(title = "svm")]
    Svm(OpportunityCreateSvm),
}

#[derive(Serialize, Deserialize, ToResponse, ToSchema, Clone)]
#[serde(untagged)]
pub enum Opportunity {
    Evm(OpportunityEvm),
    Svm(OpportunitySvm),
}

fn default_opportunity_mode() -> OpportunityMode {
    OpportunityMode::Live
}
#[derive(Clone, Serialize, Deserialize, IntoParams)]
pub struct GetOpportunitiesQueryParams {
    #[param(example = "op_sepolia", value_type = Option < String >)]
    pub chain_id:       Option<ChainId>,
    /// Get opportunities in live or historical mode
    #[param(default = "live")]
    #[serde(default = "default_opportunity_mode")]
    pub mode:           OpportunityMode,
    /// The permission key to filter the opportunities by. Used only in historical mode.
    #[param(example = "0xdeadbeef", value_type = Option< String >)]
    pub permission_key: Option<Bytes>,
    /// The time to get the opportunities from. Used only in historical mode.
    #[param(example="2024-05-23T21:26:57.329954Z", value_type = Option<String>)]
    #[serde(default, with = "crate::serde::nullable_datetime")]
    pub from_time:      Option<OffsetDateTime>,
}

// ----- Evm types -----
#[derive(Serialize, Deserialize, ToSchema, Clone)]
pub struct OpportunityBidEvm {
    /// The opportunity permission key
    #[schema(example = "0xdeadbeefcafe", value_type=String)]
    pub permission_key: PermissionKey,
    /// The bid amount in wei.
    #[schema(example = "1000000000000000000", value_type=String)]
    #[serde(with = "crate::serde::u256")]
    pub amount:         U256,
    /// The latest unix timestamp in seconds until which the bid is valid
    #[schema(example = "1000000000000000000", value_type=String)]
    #[serde(with = "crate::serde::u256")]
    pub deadline:       U256,
    /// The nonce of the bid permit signature
    #[schema(example = "123", value_type=String)]
    #[serde(with = "crate::serde::u256")]
    pub nonce:          U256,
    /// Executor address
    #[schema(example = "0x5FbDB2315678afecb367f032d93F642f64180aa2", value_type=String)]
    pub executor:       Address,
    #[schema(
        example = "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef12",
        value_type=String
    )]
    #[serde(with = "crate::serde::signature")]
    pub signature:      Signature,
}

#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug, ToResponse)]
pub struct TokenAmountEvm {
    /// Token contract address
    #[schema(example = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2", value_type = String)]
    pub token:  ethers::abi::Address,
    /// Token amount
    #[schema(example = "1000", value_type = String)]
    #[serde(with = "crate::serde::u256")]
    pub amount: U256,
}

/// Opportunity parameters needed for on-chain execution
/// If a searcher signs the opportunity and have approved enough tokens to opportunity adapter,
/// by calling this target contract with the given target calldata and structures, they will
/// send the tokens specified in the sell_tokens field and receive the tokens specified in the buy_tokens field.
#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug, ToResponse)]
pub struct OpportunityCreateV1Evm {
    /// The permission key required for successful execution of the opportunity.
    #[schema(example = "0xdeadbeefcafe", value_type = String)]
    pub permission_key:    Bytes,
    /// The chain id where the opportunity will be executed.
    #[schema(example = "op_sepolia", value_type = String)]
    pub chain_id:          String,
    /// The contract address to call for execution of the opportunity.
    #[schema(example = "0xcA11bde05977b3631167028862bE2a173976CA11", value_type = String)]
    pub target_contract:   ethers::abi::Address,
    /// Calldata for the target contract call.
    #[schema(example = "0xdeadbeef", value_type = String)]
    pub target_calldata:   Bytes,
    /// The value to send with the contract call.
    #[schema(example = "1", value_type = String)]
    #[serde(with = "crate::serde::u256")]
    pub target_call_value: U256,

    pub sell_tokens: Vec<TokenAmountEvm>,
    pub buy_tokens:  Vec<TokenAmountEvm>,
}

#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug, ToResponse)]
#[serde(tag = "version")]
pub enum OpportunityCreateEvm {
    #[serde(rename = "v1")]
    #[schema(title = "v1")]
    V1(OpportunityCreateV1Evm),
}


#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug, ToResponse)]
pub struct OpportunityParamsV1Evm(pub OpportunityCreateV1Evm);

#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug, ToResponse)]
#[serde(tag = "version")]
pub enum OpportunityParamsEvm {
    #[serde(rename = "v1")]
    #[schema(title = "v1")]
    V1(OpportunityParamsV1Evm),
}

#[derive(Serialize, Deserialize, ToSchema, Clone, ToResponse)]
pub struct OpportunityEvm {
    /// The opportunity unique id
    #[schema(example = "obo3ee3e-58cc-4372-a567-0e02b2c3d479", value_type = String)]
    pub opportunity_id: OpportunityId,
    /// Creation time of the opportunity (in microseconds since the Unix epoch)
    #[schema(example = 1_700_000_000_000_000i128, value_type = i128)]
    pub creation_time:  UnixTimestampMicros,
    #[serde(flatten)]
    #[schema(inline)]
    pub params:         OpportunityParamsEvm,
}

// ----- Svm types -----
#[serde_as]
#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug, ToResponse)]
pub struct TokenAmountSvm {
    /// Token contract address
    #[schema(example = "DUcTi3rDyS5QEmZ4BNRBejtArmDCWaPYGfN44vBJXKL5", value_type = String)]
    #[serde_as(as = "DisplayFromStr")]
    pub token:  Pubkey,
    /// Token amount in lamports
    #[schema(example = 1000)]
    pub amount: u64,
}

/// Parameters needed to create a new opportunity from the Phantom wallet
/// Auction server will extract the output token price for the auction
#[serde_as]
#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug)]
pub struct QuoteRequestCreatePhantomV1Svm {
    /// The user wallet address which requested the quote from the wallet
    #[schema(example = "DUcTi3rDyS5QEmZ4BNRBejtArmDCWaPYGfN44vBJXKL5", value_type = String)]
    #[serde_as(as = "DisplayFromStr")]
    user_wallet_address:         Pubkey,
    /// The token mint address of the input token
    #[schema(example = "DUcTi3rDyS5QEmZ4BNRBejtArmDCWaPYGfN44vBJXKL5", value_type = String)]
    #[serde_as(as = "DisplayFromStr")]
    input_token_mint:            Pubkey,
    /// The token mint address of the output token
    #[schema(example = "DUcTi3rDyS5QEmZ4BNRBejtArmDCWaPYGfN44vBJXKL5", value_type = String)]
    #[serde_as(as = "DisplayFromStr")]
    output_token_mint:           Pubkey,
    /// The input token amount that the user wants to swap
    #[schema(example = 100, value_type = u64)]
    input_token_amount:          u64,
    /// The maximum slippage percentage that the user is willing to accept
    #[schema(example = 0.5, value_type = f64)]
    maximum_slippage_percentage: f64,
}

#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug)]
#[serde(tag = "program")]
pub enum QuoteRequestCreateV1Svm {
    #[serde(rename = "phantom")]
    #[schema(title = "phantom")]
    Phantom(QuoteRequestCreatePhantomV1Svm),
}

#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug)]
#[serde(tag = "version")]
pub enum QuoteRequestCreateSvm {
    #[serde(rename = "v1")]
    #[schema(title = "v1")]
    V1(QuoteRequestCreateV1Svm),
}

#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug)]
#[serde(untagged)]
pub enum QuoteRequestCreate {
    #[schema(title = "svm")]
    Svm(QuoteRequestCreateSvm),
}

/// Program specific parameters for the opportunity
#[serde_as]
#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug)]
#[serde(tag = "program")]
pub enum OpportunityCreateProgramParamsV1Svm {
    /// Limo program specific parameters for the opportunity
    /// It contains the Limo order to be executed, encoded in base64
    /// SDKs will decode this order and create transaction for bidding on the opportunity
    #[serde(rename = "limo")]
    #[schema(title = "limo")]
    Limo {
        /// The Limo order to be executed, encoded in base64
        #[schema(example = "DUcTi3rDyS5QEmZ4BNRBejtArmDCWaPYGfN44vBJXKL5", value_type = String)]
        #[serde_as(as = "Base64")]
        order: Vec<u8>,

        /// Address of the order account
        #[schema(example = "DUcTi3rDyS5QEmZ4BNRBejtArmDCWaPYGfN44vBJXKL5", value_type = String)]
        #[serde_as(as = "DisplayFromStr")]
        order_address: Pubkey,
    },
    /// Phantom program specific parameters for the opportunity
    #[serde(rename = "phantom")]
    #[schema(title = "phantom")]
    Phantom {
        /// The user wallet address which requested the quote from the wallet
        #[schema(example = "DUcTi3rDyS5QEmZ4BNRBejtArmDCWaPYGfN44vBJXKL5", value_type = String)]
        #[serde_as(as = "DisplayFromStr")]
        user_wallet_address: Pubkey,

        /// The maximum slippage percentage that the user is willing to accept
        #[schema(example = 0.5, value_type = f64)]
        maximum_slippage_percentage: f64,
    },
}

/// Opportunity parameters needed for on-chain execution.
/// Parameters may differ for each program
#[serde_as]
#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug)]
pub struct OpportunityCreateV1Svm {
    /// The permission account to be permitted by the ER contract for the opportunity execution of the protocol
    #[schema(example = "DUcTi3rDyS5QEmZ4BNRBejtArmDCWaPYGfN44vBJXKL5", value_type = String)]
    #[serde_as(as = "DisplayFromStr")]
    pub permission_account: Pubkey,
    /// The router account to be used for the opportunity execution of the protocol
    #[schema(example = "DUcTi3rDyS5QEmZ4BNRBejtArmDCWaPYGfN44vBJXKL5", value_type = String)]
    #[serde_as(as = "DisplayFromStr")]
    pub router:             Pubkey,
    /// The chain id where the opportunity will be executed.
    #[schema(example = "solana", value_type = String)]
    pub chain_id:           ChainId,
    /// The block hash to be used for the opportunity execution
    #[schema(example = "DUcTi3rDyS5QEmZ4BNRBejtArmDCWaPYGfN44vBJXKL5", value_type = String)]
    #[serde_as(as = "DisplayFromStr")]
    pub block_hash:         Hash,
    /// The slot where the program params were fetched from using the RPC
    #[schema(example = 293106477, value_type = u64)]
    pub slot:               Slot,

    pub sell_tokens: Vec<TokenAmountSvm>,
    pub buy_tokens:  Vec<TokenAmountSvm>,

    #[serde(flatten)]
    #[schema(inline)]
    pub program_params: OpportunityCreateProgramParamsV1Svm,
}

#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug)]
#[serde(tag = "version")]
pub enum OpportunityCreateSvm {
    #[serde(rename = "v1")]
    #[schema(title = "v1")]
    V1(OpportunityCreateV1Svm),
}

#[derive(PartialEq)]
enum Program {
    Phantom,
    Limo,
}

fn get_program(auth: &Auth) -> Result<Program, RestError> {
    match auth {
        Auth::Authorized(_, profile) => {
            if profile.role == models::ProfileRole::Searcher {
                return Err(RestError::Forbidden);
            }

            match profile.name.as_str() {
                "limo" => Ok(Program::Limo),
                "phantom" => Ok(Program::Phantom),
                _ => Err(RestError::Forbidden),
            }
        }
        Auth::Admin => Err(RestError::Forbidden),
        Auth::Unauthorized => Err(RestError::Unauthorized),
    }
}

impl OpportunityCreateSvm {
    fn get_program(&self) -> Program {
        match self {
            OpportunityCreateSvm::V1(params) => match &params.program_params {
                OpportunityCreateProgramParamsV1Svm::Limo { .. } => Program::Limo,
                OpportunityCreateProgramParamsV1Svm::Phantom { .. } => Program::Phantom,
            },
        }
    }
}

/// Program specific parameters for the opportunity
#[serde_as]
#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug, ToResponse)]
#[serde(tag = "program")]
pub enum OpportunityParamsV1ProgramSvm {
    /// Limo program specific parameters for the opportunity
    /// It contains the Limo order to be executed, encoded in base64
    /// SDKs will decode this order and create transaction for bidding on the opportunity
    #[serde(rename = "limo")]
    #[schema(title = "limo")]
    Limo {
        /// The Limo order to be executed, encoded in base64
        #[schema(example = "DUcTi3rDyS5QEmZ4BNRBejtArmDCWaPYGfN44vBJXKL5", value_type = String)]
        #[serde_as(as = "Base64")]
        order:         Vec<u8>,
        /// Address of the order account
        #[schema(example = "DUcTi3rDyS5QEmZ4BNRBejtArmDCWaPYGfN44vBJXKL5", value_type = String)]
        #[serde_as(as = "DisplayFromStr")]
        order_address: Pubkey,
    },
    /// Phantom program specific parameters for the opportunity
    #[serde(rename = "phantom")]
    #[schema(title = "phantom")]
    Phantom {
        /// The user wallet address which requested the quote from the wallet
        #[schema(example = "DUcTi3rDyS5QEmZ4BNRBejtArmDCWaPYGfN44vBJXKL5", value_type = String)]
        #[serde_as(as = "DisplayFromStr")]
        user_wallet_address: Pubkey,

        /// The maximum slippage percentage that the user is willing to accept
        #[schema(example = 0.5, value_type = f64)]
        maximum_slippage_percentage: f64,
    },
}

/// Opportunity parameters needed for on-chain execution.
/// Parameters may differ for each program
#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug, ToResponse)]
pub struct OpportunityParamsV1Svm {
    #[serde(flatten)]
    #[schema(inline)]
    pub program:  OpportunityParamsV1ProgramSvm,
    #[schema(example = "solana", value_type = String)]
    pub chain_id: ChainId,
}

#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug, ToResponse)]
#[serde(tag = "version")]
pub enum OpportunityParamsSvm {
    #[serde(rename = "v1")]
    #[schema(title = "v1")]
    V1(OpportunityParamsV1Svm),
}

#[serde_as]
#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug, ToResponse)]
pub struct OpportunitySvm {
    /// The opportunity unique id
    #[schema(example = "obo3ee3e-58cc-4372-a567-0e02b2c3d479", value_type = String)]
    pub opportunity_id: OpportunityId,
    /// Creation time of the opportunity (in microseconds since the Unix epoch)
    #[schema(example = 1_700_000_000_000_000i128, value_type = i128)]
    pub creation_time:  UnixTimestampMicros,
    /// The block hash to be used for the opportunity execution
    #[schema(example = "DUcTi3rDyS5QEmZ4BNRBejtArmDCWaPYGfN44vBJXKL5", value_type = String)]
    #[serde_as(as = "DisplayFromStr")]
    pub block_hash:     Hash,
    /// The slot where the program params were fetched from using the RPC
    #[schema(example = 293106477, value_type = u64)]
    pub slot:           Slot,

    #[serde(flatten)]
    #[schema(inline)]
    pub params: OpportunityParamsSvm,
}

// ----- Implementations -----
impl OpportunityEvm {
    pub fn get_chain_id(&self) -> &ChainId {
        match &self.params {
            OpportunityParamsEvm::V1(params) => &params.0.chain_id,
        }
    }
}

impl OpportunitySvm {
    pub fn get_chain_id(&self) -> &ChainId {
        match &self.params {
            OpportunityParamsSvm::V1(params) => &params.chain_id,
        }
    }
}

impl Opportunity {
    pub fn get_chain_id(&self) -> &ChainId {
        match self {
            Opportunity::Evm(opportunity) => opportunity.get_chain_id(),
            Opportunity::Svm(opportunity) => opportunity.get_chain_id(),
        }
    }

    pub fn creation_time(&self) -> UnixTimestampMicros {
        match self {
            Opportunity::Evm(opportunity) => opportunity.creation_time,
            Opportunity::Svm(opportunity) => opportunity.creation_time,
        }
    }
}

// ----- APIs -----

/// Bid on opportunity
#[utoipa::path(post, path = "/v1/opportunities/{opportunity_id}/bids", request_body = OpportunityBidEvm,
params(("opportunity_id" = String, description = "Opportunity id to bid on")), responses(
(status = 200, description = "Bid Result", body = OpportunityBidResult, example = json ! ({"status": "OK"})),
(status = 400, response = ErrorBodyResponse),
(status = 404, description = "Opportunity or chain id was not found", body = ErrorBodyResponse),
),)]
pub async fn opportunity_bid(
    auth: Auth,
    State(store): State<Arc<StoreNew>>,
    Path(opportunity_id): Path<OpportunityId>,
    Json(opportunity_bid): Json<OpportunityBidEvm>,
) -> Result<Json<OpportunityBidResult>, RestError> {
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
        Ok(id) => Ok(OpportunityBidResult {
            status: "OK".to_string(),
            id,
        }
        .into()),
        Err(e) => Err(e),
    }
}

/// Submit an opportunity ready to be executed.
///
/// The opportunity will be verified by the server. If the opportunity is valid, it will be stored in the database
/// and will be available for bidding.
#[utoipa::path(post, path = "/v1/opportunities", request_body = OpportunityCreate, responses(
    (status = 200, description = "The created opportunity", body = Opportunity),
    (status = 400, response = ErrorBodyResponse),
    (status = 404, description = "Chain id was not found", body = ErrorBodyResponse),
),)]
pub async fn post_opportunity(
    auth: Auth,
    State(store): State<Arc<StoreNew>>,
    Json(params): Json<OpportunityCreate>,
) -> Result<Json<Opportunity>, RestError> {
    let opportunity_with_metadata: Opportunity = match params {
        OpportunityCreate::Evm(params) => store
            .opportunity_service_evm
            .add_opportunity(AddOpportunityInput {
                opportunity: params.into(),
            })
            .await?
            .into(),
        OpportunityCreate::Svm(params) => {
            if get_program(&auth)? != params.get_program() {
                return Err(RestError::Forbidden);
            }

            store
                .opportunity_service_svm
                .add_opportunity(AddOpportunityInput {
                    opportunity: params.into(),
                })
                .await?
                .into()
        }
    };
    Ok(opportunity_with_metadata.into())
}

/// Fetch opportunities ready for execution or historical opportunities
/// depending on the mode. You need to provide `chain_id` for historical mode.
/// Opportunities are sorted by creation time in ascending order in historical mode.
/// Total number of opportunities returned is limited by 20.
#[utoipa::path(get, path = "/v1/opportunities", responses(
(status = 200, description = "Array of opportunities ready for bidding", body = Vec < Opportunity >),
(status = 400, response = ErrorBodyResponse),
(status = 404, description = "Chain id was not found", body = ErrorBodyResponse),
),
params(GetOpportunitiesQueryParams))]
pub async fn get_opportunities(
    State(store): State<Arc<StoreNew>>,
    query_params: Query<GetOpportunitiesQueryParams>,
) -> Result<axum::Json<Vec<Opportunity>>, RestError> {
    let opportunities_evm = store
        .opportunity_service_evm
        .get_opportunities(GetOpportunitiesInput {
            query_params: query_params.clone().0,
        })
        .await;
    let opportunities_svm = store
        .opportunity_service_svm
        .get_opportunities(GetOpportunitiesInput {
            query_params: query_params.0,
        })
        .await;

    if opportunities_evm.is_err() && opportunities_svm.is_err() {
        // TODO better error handling, if the chain_id is svm and we have some serious error there, we would just return chain_id is not found on evm side
        Err(opportunities_evm.expect_err("Failed to get error from opportunities_evm"))
    } else {
        let mut opportunities: Vec<Opportunity> = vec![];
        if let Ok(opportunities_evm) = opportunities_evm {
            opportunities.extend(
                opportunities_evm
                    .into_iter()
                    .map(|o| o.into())
                    .collect::<Vec<Opportunity>>(),
            );
        }
        if let Ok(opportunities_svm) = opportunities_svm {
            opportunities.extend(
                opportunities_svm
                    .into_iter()
                    .map(|o| o.into())
                    .collect::<Vec<Opportunity>>(),
            );
        }

        opportunities.sort_by_key(|a| a.creation_time());
        Ok(Json(
            opportunities
                .into_iter()
                .take(repository::OPPORTUNITY_PAGE_SIZE as usize)
                .collect(),
        ))
    }
}

/// Submit a quote request
///
/// The server will estimate the quote price, which will be used to create an opportunity.
/// This opportunity, containing the estimated price, will then be submitted for bidding.
/// Once all searcher bids are collected, the winning signed bid will be returned along with the estimated price.
#[utoipa::path(post, path = "/v1/quote_request", request_body = QuoteRequestCreate, responses(
    (status = 200, description = "The created opportunity", body = Opportunity),
    (status = 400, response = ErrorBodyResponse),
    (status = 404, description = "Chain id was not found", body = ErrorBodyResponse),
),)]
pub async fn post_quote_request(
    auth: Auth,
    State(store): State<Arc<StoreNew>>,
    Json(params): Json<QuoteRequestCreate>,
) -> Result<Json<Opportunity>, RestError> {
    if get_program(&auth)? != Program::Phantom {
        return Err(RestError::Forbidden);
    }

    let QuoteRequestCreate::Svm(QuoteRequestCreateSvm::V1(QuoteRequestCreateV1Svm::Phantom(
        params,
    ))) = params;

    let opportunity = OpportunityCreateSvm::V1(OpportunityCreateV1Svm {
        permission_account: params.user_wallet_address,
        router:             Pubkey::default(),
        chain_id:           ChainId::default(),
        // TODO should be removed later
        block_hash:         Hash::default(),
        // TODO should be removed later
        slot:               Slot::default(),
        sell_tokens:        vec![TokenAmountSvm {
            token:  params.input_token_mint,
            amount: params.input_token_amount,
        }],
        buy_tokens:         vec![TokenAmountSvm {
            token:  params.output_token_mint,
            amount: store
                .opportunity_service_svm
                .estimate_price(EstimatePriceInput {
                    input_token_mint:            params.input_token_mint,
                    output_token_mint:           params.output_token_mint,
                    input_token_amount:          params.input_token_amount,
                    maximum_slippage_percentage: params.maximum_slippage_percentage,
                })
                .await?,
        }],
        program_params:     OpportunityCreateProgramParamsV1Svm::Phantom {
            user_wallet_address:         params.user_wallet_address,
            maximum_slippage_percentage: params.maximum_slippage_percentage,
        },
    });

    Ok(Json(
        store
            .opportunity_service_svm
            .add_opportunity(AddOpportunityInput {
                opportunity: opportunity.into(),
            })
            .await?
            .into(),
    ))
}

pub fn get_routes() -> Router<Arc<StoreNew>> {
    Router::new()
        .route("/", post(post_opportunity))
        .route("/phantom", post(post_quote_request))
        .route("/", get(get_opportunities))
        .route("/:opportunity_id/bids", post(opportunity_bid))
}
