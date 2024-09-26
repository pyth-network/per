use {
    super::{
        entities,
        service::{
            add_opportunity::AddOpportunityInput,
            get_opportunities::GetOpportunitiesInput,
            handle_opportunity_bid::HandleOpportunityBidInput,
            ChainType,
            Service,
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
        serde_as,
        DisplayFromStr,
    },
    solana_sdk::pubkey::Pubkey,
    std::sync::Arc,
    time::OffsetDateTime,
    utoipa::{
        IntoParams,
        ToResponse,
        ToSchema,
    },
    uuid::Uuid,
};


pub type OpportunityId = Uuid;

#[derive(Serialize, Deserialize, ToResponse, ToSchema, Clone)]
pub struct OpportunityBidResult {
    #[schema(example = "OK")]
    pub status: String,
    /// The unique id created to identify the bid. This id can be used to query the status of the bid.
    #[schema(example = "beedbeed-58cc-4372-a567-0e02b2c3d479", value_type=String)]
    pub id:     BidId,
}

#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug)]
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
#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug)]
pub struct OpportunityParamsV1Evm {
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

#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug)]
#[serde(tag = "version")]
pub enum OpportunityParamsEvm {
    #[serde(rename = "v1")]
    V1(OpportunityParamsV1Evm),
}

#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug)]
#[serde(untagged)]
pub enum OpportunityParams {
    Evm(OpportunityParamsEvm),
    Svm(OpportunityParamsSvm),
}

#[serde_as]
#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug)]
pub struct TokenAmountSvm {
    /// Token contract address
    #[schema(example = "DUcTi3rDyS5QEmZ4BNRBejtArmDCWaPYGfN44vBJXKL5", value_type = String)]
    #[serde_as(as = "DisplayFromStr")]
    pub token:  Pubkey,
    /// Token amount
    #[schema(example = 1000)]
    pub amount: u64,
}

// TODO descibe it
#[serde_as]
#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug)]
pub struct OpportunityParamsV1KaminoSvm {
    #[schema(example = "DUcTi3rDyS5QEmZ4BNRBejtArmDCWaPYGfN44vBJXKL5", value_type = String)]
    #[serde_as(as = "DisplayFromStr")]
    pub permission: Pubkey,
    #[schema(example = "DUcTi3rDyS5QEmZ4BNRBejtArmDCWaPYGfN44vBJXKL5", value_type = String)]
    #[serde_as(as = "DisplayFromStr")]
    pub router:     Pubkey,
    #[schema(example = "solana", value_type = String)]
    pub chain_id:   ChainId,

    pub sell_tokens: Vec<TokenAmountSvm>,
    pub buy_tokens:  Vec<TokenAmountSvm>,
}

#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug)]
#[serde(tag = "client")]
pub enum OpportunityParamsV1Svm {
    #[serde(rename = "kamino")]
    Kamino(OpportunityParamsV1KaminoSvm),
}

#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug)]
#[serde(tag = "version")]
pub enum OpportunityParamsSvm {
    #[serde(rename = "v1")]
    V1(OpportunityParamsV1Svm),
}

/// Similar to OpportunityParams, but with the opportunity id included.
#[derive(Serialize, Deserialize, ToSchema, Clone, ToResponse)]
pub struct OpportunityParamsWithMetadataEvm {
    /// The opportunity unique id
    #[schema(example = "obo3ee3e-58cc-4372-a567-0e02b2c3d479", value_type = String)]
    opportunity_id: OpportunityId,
    /// Creation time of the opportunity (in microseconds since the Unix epoch)
    #[schema(example = 1_700_000_000_000_000i128, value_type = i128)]
    creation_time:  UnixTimestampMicros,
    /// opportunity data
    #[serde(flatten)]
    // expands params into component fields in the generated client schemas
    #[schema(inline)]
    params:         OpportunityParamsEvm,
}

#[derive(Serialize, Deserialize, ToSchema, Clone, ToResponse)]
#[serde(untagged)]
pub enum OpportunityParamsWithMetadata {
    Evm(OpportunityParamsWithMetadataEvm),
    Svm(OpportunityParamsWithMetadataSvm),
}

impl From<entities::OpportunityEvm> for OpportunityParamsWithMetadata {
    fn from(val: entities::OpportunityEvm) -> Self {
        OpportunityParamsWithMetadata::Evm(val.into())
    }
}

impl From<entities::OpportunitySvm> for OpportunityParamsWithMetadata {
    fn from(val: entities::OpportunitySvm) -> Self {
        OpportunityParamsWithMetadata::Svm(val.into())
    }
}

/// Similar to OpportunityParams, but with the opportunity id included.
#[derive(Serialize, Deserialize, ToSchema, Clone, ToResponse)]
pub struct OpportunityParamsWithMetadataSvm {
    /// The opportunity unique id
    #[schema(example = "obo3ee3e-58cc-4372-a567-0e02b2c3d479", value_type = String)]
    opportunity_id: OpportunityId,
    /// Creation time of the opportunity (in microseconds since the Unix epoch)
    #[schema(example = 1_700_000_000_000_000i128, value_type = i128)]
    creation_time:  UnixTimestampMicros,
    /// opportunity data
    #[serde(flatten)]
    // expands params into component fields in the generated client schemas
    #[schema(inline)]
    params:         OpportunityParamsSvm,
}

impl OpportunityParamsWithMetadataEvm {
    pub fn get_chain_id(&self) -> &ChainId {
        match &self.params {
            OpportunityParamsEvm::V1(params) => &params.chain_id,
        }
    }
}

impl OpportunityParamsWithMetadataSvm {
    pub fn get_chain_id(&self) -> &ChainId {
        match &self.params {
            OpportunityParamsSvm::V1(params) => match params {
                OpportunityParamsV1Svm::Kamino(params) => &params.chain_id,
            },
        }
    }
}

impl OpportunityParamsWithMetadata {
    pub fn get_chain_id(&self) -> &ChainId {
        match self {
            OpportunityParamsWithMetadata::Evm(opportunity) => opportunity.get_chain_id(),
            OpportunityParamsWithMetadata::Svm(opportunity) => opportunity.get_chain_id(),
        }
    }
}

impl From<entities::TokenAmountEvm> for TokenAmountEvm {
    fn from(val: entities::TokenAmountEvm) -> Self {
        TokenAmountEvm {
            token:  val.token,
            amount: val.amount,
        }
    }
}

impl From<entities::TokenAmountSvm> for TokenAmountSvm {
    fn from(val: entities::TokenAmountSvm) -> Self {
        TokenAmountSvm {
            token:  val.token,
            amount: val.amount,
        }
    }
}

impl From<entities::OpportunityEvm> for OpportunityParamsWithMetadataEvm {
    fn from(val: entities::OpportunityEvm) -> Self {
        OpportunityParamsWithMetadataEvm {
            opportunity_id: val.id,
            creation_time:  val.creation_time,
            params:         OpportunityParamsEvm::V1(OpportunityParamsV1Evm {
                permission_key:    val.permission_key.clone(),
                chain_id:          val.chain_id.clone(),
                target_contract:   val.target_contract,
                target_calldata:   val.target_calldata.clone(),
                target_call_value: val.target_call_value,
                sell_tokens:       val
                    .sell_tokens
                    .clone()
                    .into_iter()
                    .map(|t| t.into())
                    .collect(),
                buy_tokens:        val
                    .buy_tokens
                    .clone()
                    .into_iter()
                    .map(|t| t.into())
                    .collect(),
            }),
        }
    }
}

impl From<entities::OpportunitySvm> for OpportunityParamsWithMetadataSvm {
    fn from(val: entities::OpportunitySvm) -> Self {
        OpportunityParamsWithMetadataSvm {
            opportunity_id: val.id,
            creation_time:  val.creation_time,
            params:         OpportunityParamsSvm::V1(OpportunityParamsV1Svm::Kamino(
                OpportunityParamsV1KaminoSvm {
                    permission:  val.permission.clone(),
                    chain_id:    val.chain_id.clone(),
                    router:      val.router.clone(),
                    sell_tokens: val
                        .sell_tokens
                        .clone()
                        .into_iter()
                        .map(|t| t.into())
                        .collect(),
                    buy_tokens:  val
                        .buy_tokens
                        .clone()
                        .into_iter()
                        .map(|t| t.into())
                        .collect(),
                },
            )),
        }
    }
}

impl From<TokenAmountEvm> for entities::TokenAmountEvm {
    fn from(val: TokenAmountEvm) -> Self {
        entities::TokenAmountEvm {
            token:  val.token,
            amount: val.amount,
        }
    }
}

impl From<TokenAmountSvm> for entities::TokenAmountSvm {
    fn from(val: TokenAmountSvm) -> Self {
        entities::TokenAmountSvm {
            token:  val.token,
            amount: val.amount,
        }
    }
}

impl From<OpportunityParamsV1Evm> for entities::OpportunityEvm {
    fn from(val: OpportunityParamsV1Evm) -> Self {
        let id = Uuid::new_v4();
        let now_odt = OffsetDateTime::now_utc();
        entities::OpportunityEvm {
            core_fields:       entities::OpportunityCoreFields::<entities::TokenAmountEvm> {
                id,
                permission_key: val.permission_key.clone(),
                chain_id: val.chain_id.clone(),
                sell_tokens: val.sell_tokens.into_iter().map(|t| t.into()).collect(),
                buy_tokens: val.buy_tokens.into_iter().map(|t| t.into()).collect(),
                creation_time: now_odt.unix_timestamp_nanos() / 1000 as UnixTimestampMicros,
            },
            target_contract:   val.target_contract,
            target_calldata:   val.target_calldata,
            target_call_value: val.target_call_value,
        }
    }
}


impl From<OpportunityParamsV1Svm> for entities::OpportunitySvm {
    fn from(val: OpportunityParamsV1Svm) -> Self {
        let val = match val {
            OpportunityParamsV1Svm::Kamino(val) => val,
        };
        let id = Uuid::new_v4();
        let now_odt = OffsetDateTime::now_utc();
        entities::OpportunitySvm {
            core_fields: entities::OpportunityCoreFields::<entities::TokenAmountSvm> {
                id,
                permission_key: [val.permission.to_bytes(), val.router.to_bytes()]
                    .concat()
                    .into(),
                chain_id: val.chain_id,
                sell_tokens: val.sell_tokens.into_iter().map(|t| t.into()).collect(),
                buy_tokens: val.buy_tokens.into_iter().map(|t| t.into()).collect(),
                creation_time: now_odt.unix_timestamp_nanos() / 1000 as UnixTimestampMicros,
            },
            permission:  val.permission,
            router:      val.router,
        }
    }
}


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
#[utoipa::path(post, path = "/v1/opportunities", request_body = OpportunityParams, responses(
(status = 200, description = "The created opportunity", body = OpportunityParamsWithMetadata),
(status = 400, response = ErrorBodyResponse),
(status = 404, description = "Chain id was not found", body = ErrorBodyResponse),
),)]
pub async fn post_opportunity(
    State(store): State<Arc<StoreNew>>,
    Json(params): Json<OpportunityParams>,
) -> Result<Json<OpportunityParamsWithMetadata>, RestError> {
    let opportunity_with_metadata: OpportunityParamsWithMetadata = match params {
        OpportunityParams::Evm(versioned_params) => {
            let OpportunityParamsEvm::V1(params) = versioned_params.clone();
            // TODO Need a new entity for CreateOpportunity
            store
                .opportunity_service_evm
                .add_opportunity(AddOpportunityInput {
                    opportunity: params.into(),
                })
                .await?
                .into()
        }
        OpportunityParams::Svm(versioned_params) => {
            let OpportunityParamsSvm::V1(params) = versioned_params.clone();
            // TODO Need a new entity for CreateOpportunity
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


#[derive(Debug, Serialize, Deserialize, ToSchema, Clone)]
#[serde(rename_all = "lowercase")]
pub enum OpportunityMode {
    Live,
    Historical,
}

fn default_opportunity_mode() -> OpportunityMode {
    OpportunityMode::Live
}

#[derive(Serialize, Deserialize, IntoParams)]
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

/// Fetch opportunities ready for execution or historical opportunities
/// depending on the mode. You need to provide `chain_id` for historical mode.
/// Opportunities are sorted by creation time in ascending order in historical mode.
#[utoipa::path(get, path = "/v1/opportunities", responses(
(status = 200, description = "Array of opportunities ready for bidding", body = Vec < OpportunityParamsWithMetadataEvm >),
(status = 400, response = ErrorBodyResponse),
(status = 404, description = "Chain id was not found", body = ErrorBodyResponse),
),
params(GetOpportunitiesQueryParams))]
pub async fn get_opportunities(
    State(store): State<Arc<StoreNew>>,
    query_params: Query<GetOpportunitiesQueryParams>,
) -> Result<axum::Json<Vec<OpportunityParamsWithMetadataEvm>>, RestError> {
    let opportunities = store
        .opportunity_service_evm
        .get_opportunities(GetOpportunitiesInput {
            query_params: query_params.0,
        })
        .await?;
    Ok(Json(opportunities.into_iter().map(|o| o.into()).collect()))
}

pub fn get_routes() -> Router<Arc<StoreNew>> {
    Router::new()
        .route("/", post(post_opportunity))
        .route("/", get(get_opportunities))
        .route("/:opportunity_id/bids", post(opportunity_bid))
}
