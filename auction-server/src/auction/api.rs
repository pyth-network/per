use {
    super::{
        entities::{
            self,
            BidChainData,
        },
        repository::MicroLamports,
        service::{
            get_bid::GetBidInput,
            get_bids::GetBidsInput,
            handle_bid::HandleBidInput,
            verification::Verification,
            ChainTrait,
            Service,
            ServiceEnum,
        },
    },
    crate::{
        api::{
            require_login_middleware,
            Auth,
            ErrorBodyResponse,
            RestError,
        },
        kernel::entities::{
            ChainId,
            Evm,
            PermissionKey,
            PermissionKeySvm,
            Svm,
        },
        login_required,
        models,
        state::StoreNew,
    },
    axum::{
        async_trait,
        extract::{
            Path,
            Query,
            State,
        },
        middleware,
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
        H256,
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
    solana_sdk::{
        clock::Slot,
        hash::Hash,
        signature::Signature,
        transaction::VersionedTransaction,
    },
    sqlx::types::time::OffsetDateTime,
    std::{
        fmt::Debug,
        sync::Arc,
    },
    utoipa::{
        IntoParams,
        ToResponse,
        ToSchema,
    },
    uuid::Uuid,
};

// TODO move it to kernel?

#[serde_as]
#[derive(Serialize, Clone, ToSchema, ToResponse)]
pub struct SvmChainUpdate {
    #[schema(example = "solana", value_type = String)]
    pub chain_id:                  ChainId,
    #[serde_as(as = "DisplayFromStr")]
    #[schema(example = "SLxp9LxX1eE9Z5v99Y92DaYEwyukFgMUF6zRerCF12j", value_type = String)]
    pub blockhash:                 Hash,
    /// The prioritization fee that the server suggests to use for the next transaction
    #[schema(example = "1000", value_type = u64)]
    pub latest_prioritization_fee: MicroLamports,
}

pub type BidId = Uuid;

#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BidStatusEvm {
    /// The temporary state which means the auction for this bid is pending.
    #[schema(title = "Pending")]
    Pending,
    /// The bid is submitted to the chain, which is placed at the given index of the transaction with the given hash.
    /// This state is temporary and will be updated to either lost or won after conclusion of the auction.
    #[schema(title = "Submitted")]
    Submitted {
        #[schema(example = "0x103d4fbd777a36311b5161f2062490f761f25b67406badb2bace62bb170aa4e3", value_type = String)]
        result: H256,
        #[schema(example = 1, value_type = u32)]
        index:  u32,
    },
    /// The bid lost the auction, which is concluded with the transaction with the given hash and index.
    /// The result will be None if the auction was concluded off-chain and no auction was submitted to the chain.
    /// The index will be None if the bid was not submitted to the chain and lost the auction by off-chain calculation.
    /// There are cases where the result is not None and the index is None.
    /// It is because other bids were selected for submission to the chain, but not this one.
    #[schema(title = "Lost")]
    Lost {
        #[schema(example = "0x103d4fbd777a36311b5161f2062490f761f25b67406badb2bace62bb170aa4e3", value_type = Option<String>)]
        result: Option<H256>,
        #[schema(example = 1, value_type = Option<u32>)]
        index:  Option<u32>,
    },
    /// The bid won the auction, which is concluded with the transaction with the given hash and index.
    #[schema(title = "Won")]
    Won {
        #[schema(example = "0x103d4fbd777a36311b5161f2062490f761f25b67406badb2bace62bb170aa4e3", value_type = String)]
        result: H256,
        #[schema(example = 1, value_type = u32)]
        index:  u32,
    },
}

#[serde_as]
#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BidStatusSvm {
    /// The temporary state which means the auction for this bid is pending.
    /// It will be updated to Lost or Submitted after the auction takes place.
    #[schema(title = "Pending")]
    Pending,
    /// The bid lost the auction.
    /// This bid status will have a result field containing the signature of the transaction corresponding to the winning bid,
    /// unless the auction had no winner (because all bids were found to be invalid).
    #[schema(title = "Lost")]
    Lost {
        #[schema(example = "Jb2urXPyEh4xiBgzYvwEFe4q1iMxG1DNxWGGQg94AmKgqFTwLAiTiHrYiYxwHUB4DV8u5ahNEVtMMDm3sNSRdTg", value_type = Option<String>)]
        #[serde(with = "crate::serde::nullable_signature_svm")]
        result: Option<Signature>,
    },
    /// The bid won the auction and was submitted to the chain, with the signature of the corresponding transaction provided in the result field.
    /// This state is temporary and will be updated to either Won or Failed after the transaction is included in a block, or Expired if the transaction expires before it is included.
    #[schema(title = "Submitted")]
    Submitted {
        #[schema(example = "Jb2urXPyEh4xiBgzYvwEFe4q1iMxG1DNxWGGQg94AmKgqFTwLAiTiHrYiYxwHUB4DV8u5ahNEVtMMDm3sNSRdTg", value_type = String)]
        #[serde_as(as = "DisplayFromStr")]
        result: Signature,
    },
    /// The bid won the auction and was included in a block successfully.
    #[schema(title = "Won")]
    Won {
        #[schema(example = "Jb2urXPyEh4xiBgzYvwEFe4q1iMxG1DNxWGGQg94AmKgqFTwLAiTiHrYiYxwHUB4DV8u5ahNEVtMMDm3sNSRdTg", value_type = String)]
        #[serde_as(as = "DisplayFromStr")]
        result: Signature,
    },
    /// The bid was submitted on-chain, was included in a block, but resulted in a failed transaction.
    #[schema(title = "Failed")]
    Failed {
        #[schema(example = "Jb2urXPyEh4xiBgzYvwEFe4q1iMxG1DNxWGGQg94AmKgqFTwLAiTiHrYiYxwHUB4DV8u5ahNEVtMMDm3sNSRdTg", value_type = String)]
        #[serde_as(as = "DisplayFromStr")]
        result: Signature,
    },
    /// The bid was submitted on-chain but expired before it was included in a block.
    #[schema(title = "Expired")]
    Expired {
        #[schema(example = "Jb2urXPyEh4xiBgzYvwEFe4q1iMxG1DNxWGGQg94AmKgqFTwLAiTiHrYiYxwHUB4DV8u5ahNEVtMMDm3sNSRdTg", value_type = String)]
        #[serde_as(as = "DisplayFromStr")]
        result: Signature,
    },
}

#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug)]
#[serde(untagged)]
pub enum BidStatus {
    Evm(BidStatusEvm),
    Svm(BidStatusSvm),
}

#[derive(Serialize, Deserialize, ToResponse, ToSchema, Clone)]
pub struct BidResult {
    /// The status of the request. If the bid was placed successfully, the status will be "OK".
    #[schema(example = "OK")]
    pub status: String,
    /// The unique id created to identify the bid. This id can be used to query the status of the bid.
    #[schema(example = "beedbeed-58cc-4372-a567-0e02b2c3d479", value_type=String)]
    pub id:     BidId,
}

#[derive(Clone, Debug, ToSchema, Serialize, Deserialize)]
pub struct BidCoreFields {
    /// The unique id for bid.
    #[schema(example = "obo3ee3e-58cc-4372-a567-0e02b2c3d479", value_type = String)]
    pub id:              BidId,
    /// The chain id for bid.
    #[schema(example = "op_sepolia", value_type = String)]
    pub chain_id:        ChainId,
    /// The time server received the bid formatted in rfc3339.
    #[schema(example = "2024-05-23T21:26:57.329954Z", value_type = String)]
    #[serde(with = "time::serde::rfc3339")]
    pub initiation_time: OffsetDateTime,
    /// The profile id for the bid owner.
    #[schema(example = "obo3ee3e-58cc-4372-a567-0e02b2c3d479", value_type = String)]
    pub profile_id:      Option<models::ProfileId>,
}

#[derive(Clone, Debug, ToSchema, Serialize, Deserialize)]
pub struct BidSvm {
    #[serde(flatten)]
    #[schema(inline)]
    pub core_fields:    BidCoreFields,
    /// The latest status for bid.
    pub status:         BidStatusSvm,
    /// The transaction of the bid.
    #[schema(example = "SGVsbG8sIFdvcmxkIQ==", value_type = String)]
    #[serde(with = "crate::serde::transaction_svm")]
    pub transaction:    VersionedTransaction,
    /// Amount of bid in lamports.
    #[schema(example = "1000", value_type = u64)]
    pub bid_amount:     entities::BidAmountSvm,
    /// The permission key for bid in base64 format.
    /// This is the concatenation of the permission account and the router account.
    #[schema(example = "DUcTi3rDyS5QEmZ4BNRBejtArmDCWaPYGfN44vBJXKL5", value_type = String)]
    pub permission_key: PermissionKeySvm,
}

#[derive(Clone, Debug, ToSchema, Serialize, Deserialize)]
pub struct BidEvm {
    #[serde(flatten)]
    #[schema(inline)]
    pub core_fields:     BidCoreFields,
    /// The latest status for bid.
    pub status:          BidStatusEvm,
    /// The contract address to call.
    #[schema(example = "0xcA11bde05977b3631167028862bE2a173976CA11", value_type = String)]
    pub target_contract: Address,
    /// Calldata for the contract call.
    #[schema(example = "0xdeadbeef", value_type = String)]
    pub target_calldata: Bytes,
    /// The gas limit for the contract call.
    #[schema(example = "2000000", value_type = String)]
    #[serde(with = "crate::serde::u256")]
    pub gas_limit:       U256,
    /// Amount of bid in wei.
    #[schema(example = "10", value_type = String)]
    #[serde(with = "crate::serde::u256")]
    pub bid_amount:      entities::BidAmountEvm,
    /// The permission key for bid.
    #[schema(example = "0xdeadbeef", value_type = String)]
    pub permission_key:  PermissionKey,
}

#[derive(Clone, Debug, ToSchema, Serialize, Deserialize)]
#[serde(untagged)]
#[allow(clippy::large_enum_variant)]
pub enum Bid {
    Evm(BidEvm),
    Svm(BidSvm),
}

impl Bid {
    pub fn get_initiation_time(&self) -> OffsetDateTime {
        match self {
            Bid::Evm(bid) => bid.core_fields.initiation_time,
            Bid::Svm(bid) => bid.core_fields.initiation_time,
        }
    }

    pub fn get_status(&self) -> BidStatus {
        match self {
            Bid::Evm(bid) => BidStatus::Evm(bid.status.clone()),
            Bid::Svm(bid) => BidStatus::Svm(bid.status.clone()),
        }
    }
}

#[derive(Serialize, Deserialize, ToSchema, Clone, Debug)]
pub struct BidCreateEvm {
    /// The permission key to bid on.
    #[schema(example = "0xdeadbeef", value_type = String)]
    pub permission_key:  Bytes,
    /// The chain id to bid on.
    #[schema(example = "op_sepolia", value_type = String)]
    pub chain_id:        ChainId,
    /// The contract address to call.
    #[schema(example = "0xcA11bde05977b3631167028862bE2a173976CA11", value_type = String)]
    pub target_contract: Address,
    /// Calldata for the contract call.
    #[schema(example = "0xdeadbeef", value_type = String)]
    pub target_calldata: Bytes,
    /// Amount of bid in wei.
    #[schema(example = "10", value_type = String)]
    #[serde(with = "crate::serde::u256")]
    pub amount:          entities::BidAmountEvm,
}

#[derive(Serialize, Deserialize, ToSchema, Clone, Debug)]
pub struct BidCreateSvm {
    /// The chain id to bid on.
    #[schema(example = "solana", value_type = String)]
    pub chain_id:    ChainId,
    /// The transaction for bid.
    #[schema(example = "SGVsbG8sIFdvcmxkIQ==", value_type = String)]
    #[serde(with = "crate::serde::transaction_svm")]
    pub transaction: VersionedTransaction,
    /// The minimum slot required for the bid to be executed successfully
    /// None if the bid can be executed at any recent slot
    #[schema(example = 293106477, value_type = Option<u64>)]
    pub slot:        Option<Slot>,
}

#[derive(Serialize, Deserialize, ToSchema, Debug, Clone)]
#[serde(untagged)] // Remove tags to avoid key-value wrapping
pub enum BidCreate {
    Evm(BidCreateEvm),
    Svm(BidCreateSvm),
}

#[derive(Serialize, Clone, ToSchema, ToResponse)]
pub struct BidStatusWithId {
    #[schema(value_type = String)]
    pub id:         BidId,
    pub bid_status: BidStatus,
}

/// Bid on a specific permission key for a specific chain.
///
/// Your bid will be verified by the server. Depending on the outcome of the auction, a transaction
/// containing your bid will be sent to the blockchain expecting the bid amount to be paid in the transaction.
#[utoipa::path(post, path = "/v1/bids", request_body = BidCreate, responses(
    (status = 200, description = "Bid was placed successfully", body = BidResult),
    (status = 400, response = ErrorBodyResponse),
    (status = 404, description = "Chain id was not found", body = ErrorBodyResponse),
),)]
pub async fn post_bid(
    auth: Auth,
    State(store): State<Arc<StoreNew>>,
    Json(bid_create): Json<BidCreate>,
) -> Result<Json<BidResult>, RestError> {
    process_bid(auth, store, bid_create).await
}

pub async fn process_bid(
    auth: Auth,
    store: Arc<StoreNew>,
    bid_create: BidCreate,
) -> Result<Json<BidResult>, RestError> {
    let profile = match auth {
        Auth::Authorized(_, profile) => Some(profile),
        _ => None,
    };
    match store.get_auction_service(&bid_create.get_chain_id())? {
        ServiceEnum::Evm(service) => Evm::handle_bid(&service, &bid_create, profile).await,
        ServiceEnum::Svm(service) => Svm::handle_bid(&service, &bid_create, profile).await,
    }
}

#[derive(Serialize, Deserialize, IntoParams, Clone)]
pub struct GetBidStatusParams {
    #[param(example="op_sepolia", value_type = String)]
    pub chain_id: ChainId,

    #[param(example="obo3ee3e-58cc-4372-a567-0e02b2c3d479", value_type = String)]
    pub bid_id: BidId,
}

/// Query the status of a specific bid.
#[utoipa::path(get, path = "/v1/{chain_id}/bids/{bid_id}",
    responses(
    (status = 200, body = BidStatus),
    (status = 400, response = ErrorBodyResponse),
    (status = 404, description = "Bid was not found", body = ErrorBodyResponse),
),
    params(GetBidStatusParams),
)]
pub async fn get_bid_status(
    State(store): State<Arc<StoreNew>>,
    Path(params): Path<GetBidStatusParams>,
) -> Result<Json<BidStatus>, RestError> {
    match store.get_auction_service(&params.chain_id)? {
        ServiceEnum::Evm(service) => Evm::get_bid_status(&service, params.bid_id).await,
        ServiceEnum::Svm(service) => Svm::get_bid_status(&service, params.bid_id).await,
    }
}

/// Query the status of a specific bid.
///
/// This api is deprecated and will be removed soon. Use /v1/{chain_id}/bids/{bid_id} instead.
#[utoipa::path(get, path = "/v1/bids/{bid_id}",
    params(("bid_id"=String, description = "Bid id to query for")),
    responses(
    (status = 200, body = BidStatus),
    (status = 400, response = ErrorBodyResponse),
    (status = 404, description = "Bid was not found", body = ErrorBodyResponse),
),)]
#[deprecated = "Use get_bid_status instead"]
pub async fn get_bid_status_deprecated(
    State(store): State<Arc<StoreNew>>,
    Path(bid_id): Path<BidId>,
) -> Result<Json<BidStatus>, RestError> {
    for service in store.get_all_auction_services().values() {
        let result = match service {
            ServiceEnum::Evm(service) => Evm::get_bid_status(service, bid_id).await,
            ServiceEnum::Svm(service) => Svm::get_bid_status(service, bid_id).await,
        };
        match result {
            Ok(_) => return result,
            Err(RestError::BidNotFound) => continue,
            Err(e) => return Err(e),
        }
    }

    Err(RestError::BidNotFound)
}

#[derive(Serialize, Deserialize, ToResponse, ToSchema, Clone)]
pub struct Bids {
    pub items: Vec<Bid>,
}

#[derive(Serialize, Deserialize, IntoParams)]
pub struct GetBidsByTimeQueryParams {
    #[param(example="2024-05-23T21:26:57.329954Z", value_type = Option<String>)]
    #[serde(default, with = "crate::serde::nullable_datetime")]
    pub from_time: Option<OffsetDateTime>,
}

/// Returns at most 20 bids which were submitted after a specific time and chain.
/// If no time is provided, the server will return the first bids.
#[utoipa::path(get, path = "/v1/{chain_id}/bids",
    security(
        ("bearerAuth" = []),
    ),
    responses(
    (status = 200, description = "Paginated list of bids for the specified query", body = Bids),
    (status = 400, response = ErrorBodyResponse),
),  params(
        ("chain_id"=String, Path, description = "The chain id to query for", example = "op_sepolia"),
        GetBidsByTimeQueryParams
    ),
)]
pub async fn get_bids_by_time(
    auth: Auth,
    State(store): State<Arc<StoreNew>>,
    Path(chain_id): Path<ChainId>,
    query: Query<GetBidsByTimeQueryParams>,
) -> Result<Json<Bids>, RestError> {
    match auth {
        Auth::Authorized(_, profile) => match store.get_auction_service(&chain_id)? {
            ServiceEnum::Evm(service) => {
                Evm::get_bids_by_time(&service, profile, query.from_time).await
            }
            ServiceEnum::Svm(service) => {
                Svm::get_bids_by_time(&service, profile, query.from_time).await
            }
        },
        _ => {
            tracing::error!("Unauthorized access to get_bids_by_time");
            Err(RestError::TemporarilyUnavailable)
        }
    }
}

/// Returns at most 20 bids which were submitted after a specific time.
///
/// If no time is provided, the server will return the first bids.
/// This api is deprecated and will be removed soon. Use /v1/{chain_id}/bids instead.
#[utoipa::path(get, path = "/v1/bids",
    security(
        ("bearerAuth" = []),
    ),
    responses(
    (status = 200, description = "Paginated list of bids for the specified query", body = Bids),
    (status = 400, response = ErrorBodyResponse),
),  params(GetBidsByTimeQueryParams),
)]
#[deprecated = "Use get_bids_by_time instead"]
pub async fn get_bids_by_time_deprecated(
    auth: Auth,
    State(store): State<Arc<StoreNew>>,
    query: Query<GetBidsByTimeQueryParams>,
) -> Result<Json<Bids>, RestError> {
    match auth {
        Auth::Authorized(_, profile) => {
            let mut bids: Vec<Bid> = vec![];
            for service in store.get_all_auction_services().values() {
                let new_bids = match service {
                    ServiceEnum::Evm(service) => {
                        Evm::get_bids_by_time(service, profile.clone(), query.from_time).await?
                    }
                    ServiceEnum::Svm(service) => {
                        Svm::get_bids_by_time(service, profile.clone(), query.from_time).await?
                    }
                };
                bids.extend(new_bids.items.clone());
            }
            bids.sort_by_key(|a| a.get_initiation_time());
            bids.truncate(20);
            Ok(Json(Bids { items: bids }))
        }
        _ => {
            tracing::error!("Unauthorized access to get_bids_by_time");
            Err(RestError::TemporarilyUnavailable)
        }
    }
}

pub fn get_routes(store: Arc<StoreNew>) -> Router<Arc<StoreNew>> {
    #[allow(deprecated)]
    Router::new()
        .route("/", post(post_bid))
        .route(
            "/",
            login_required!(store, get(get_bids_by_time_deprecated)),
        )
        .route("/:bid_id", get(get_bid_status_deprecated))
}

pub fn get_routes_with_chain_id(store: Arc<StoreNew>) -> Router<Arc<StoreNew>> {
    Router::new()
        .route("/", login_required!(store, get(get_bids_by_time)))
        .route("/:bid_id", get(get_bid_status))
}

impl From<entities::BidStatusEvm> for BidStatusEvm {
    fn from(status: entities::BidStatusEvm) -> Self {
        match status {
            entities::BidStatusEvm::Pending => BidStatusEvm::Pending,
            entities::BidStatusEvm::Submitted { auction, index } => BidStatusEvm::Submitted {
                result: auction.tx_hash,
                index,
            },
            entities::BidStatusEvm::Lost { auction, index } => BidStatusEvm::Lost {
                result: auction.map(|a| a.tx_hash),
                index,
            },
            entities::BidStatusEvm::Won { auction, index } => BidStatusEvm::Won {
                result: auction.tx_hash,
                index,
            },
        }
    }
}

impl From<entities::BidStatusSvm> for BidStatusSvm {
    fn from(status: entities::BidStatusSvm) -> Self {
        match status {
            entities::BidStatusSvm::Pending => BidStatusSvm::Pending,
            entities::BidStatusSvm::Submitted { auction } => BidStatusSvm::Submitted {
                result: auction.tx_hash,
            },
            entities::BidStatusSvm::Lost { auction } => BidStatusSvm::Lost {
                result: auction.map(|a| a.tx_hash),
            },
            entities::BidStatusSvm::Won { auction } => BidStatusSvm::Won {
                result: auction.tx_hash,
            },
            entities::BidStatusSvm::Failed { auction } => BidStatusSvm::Failed {
                result: auction.tx_hash,
            },
            entities::BidStatusSvm::Expired { auction } => BidStatusSvm::Expired {
                result: auction.tx_hash,
            },
        }
    }
}

// TODO switch to generic structure format
impl BidCoreFields {
    pub fn from_bid<T: ChainTrait>(bid: &entities::Bid<T>) -> Self {
        BidCoreFields {
            id:              bid.id,
            chain_id:        bid.chain_id.clone(),
            initiation_time: bid.initiation_time,
            profile_id:      bid.profile_id,
        }
    }
}

impl From<entities::Bid<Evm>> for Bid {
    fn from(bid: entities::Bid<Evm>) -> Self {
        Bid::Evm(BidEvm {
            core_fields:     BidCoreFields::from_bid(&bid),
            status:          bid.status.into(),
            permission_key:  bid.chain_data.get_permission_key(),
            target_contract: bid.chain_data.target_contract,
            target_calldata: bid.chain_data.target_calldata,
            gas_limit:       bid.chain_data.gas_limit,
            bid_amount:      bid.amount,
        })
    }
}

impl From<entities::Bid<Svm>> for Bid {
    fn from(bid: entities::Bid<Svm>) -> Self {
        Bid::Svm(BidSvm {
            core_fields:    BidCoreFields::from_bid(&bid),
            permission_key: bid.chain_data.get_permission_key(),
            status:         bid.status.into(),
            transaction:    bid.chain_data.transaction,
            bid_amount:     bid.amount,
        })
    }
}

impl BidCreate {
    fn get_chain_id(&self) -> ChainId {
        match self {
            BidCreate::Evm(bid_create_evm) => bid_create_evm.chain_id.clone(),
            BidCreate::Svm(bid_create_svm) => bid_create_svm.chain_id.clone(),
        }
    }
}

impl From<entities::BidStatusEvm> for BidStatus {
    fn from(bid: entities::BidStatusEvm) -> Self {
        BidStatus::Evm(bid.into())
    }
}

impl From<entities::BidStatusSvm> for BidStatus {
    fn from(bid: entities::BidStatusSvm) -> Self {
        BidStatus::Svm(bid.into())
    }
}

#[async_trait]
trait ApiTrait<T: ChainTrait>
where
    Service<T>: Verification<T>,
    entities::Bid<T>: Into<Bid>,
{
    type BidCreateType: Clone + Debug + Send + Sync;

    async fn handle_bid(
        service: &Service<T>,
        bid_create: &BidCreate,
        profile: Option<models::Profile>,
    ) -> Result<Json<BidResult>, RestError> {
        let bid = Self::get_bid_create_entity(bid_create, profile)?;
        let bid = service
            .handle_bid(HandleBidInput { bid_create: bid })
            .await?;
        Ok(Json(BidResult {
            status: "OK".to_string(),
            id:     bid.id,
        }))
    }

    async fn get_bid_status(
        service: &Service<T>,
        bid_id: entities::BidId,
    ) -> Result<Json<BidStatus>, RestError> {
        let bid: Bid = service.get_bid(GetBidInput { bid_id }).await?.into();
        Ok(Json(bid.get_status()))
    }

    async fn get_bids_by_time(
        service: &Service<T>,
        profile: models::Profile,
        from_time: Option<OffsetDateTime>,
    ) -> Result<Json<Bids>, RestError> {
        let bids = service
            .get_bids(GetBidsInput { profile, from_time })
            .await?;
        Ok(Json(Bids {
            items: bids.into_iter().map(|b| b.into()).collect(),
        }))
    }

    fn get_bid_create_entity(
        bid: &BidCreate,
        profile: Option<models::Profile>,
    ) -> Result<entities::BidCreate<T>, RestError>;
}

impl ApiTrait<Evm> for Evm {
    type BidCreateType = BidCreateEvm;

    fn get_bid_create_entity(
        bid: &BidCreate,
        profile: Option<models::Profile>,
    ) -> Result<entities::BidCreate<Evm>, RestError> {
        match bid {
            BidCreate::Evm(bid_create_evm) => {
                Ok(entities::BidCreate::<Evm> {
                    chain_id: bid_create_evm.chain_id.clone(),
                    profile,
                    initiation_time: OffsetDateTime::now_utc(),
                    chain_data: entities::BidChainDataCreateEvm {
                        target_contract: bid_create_evm.target_contract,
                        target_calldata: bid_create_evm.target_calldata.clone(),
                        permission_key:  bid_create_evm.permission_key.clone(),
                        amount:          bid_create_evm.amount,
                    },
                })
            }
            _ => Err(RestError::BadParameters(
                "Expected EVM chain_id. Ensure that the bid type matches the expected chain for the specified chain_id.".to_string()
            )),
        }
    }
}

impl ApiTrait<Svm> for Svm {
    type BidCreateType = BidCreateSvm;

    fn get_bid_create_entity(
        bid: &BidCreate,
        profile: Option<models::Profile>,
    ) -> Result<entities::BidCreate<Svm>, RestError> {
        match bid {
            BidCreate::Svm(bid_create_svm) => {
                Ok(entities::BidCreate::<Svm> {
                    chain_id: bid_create_svm.chain_id.clone(),
                    profile,
                    initiation_time: OffsetDateTime::now_utc(),
                    chain_data: entities::BidChainDataCreateSvm {
                        transaction: bid_create_svm.transaction.clone(),
                        slot:        bid_create_svm.slot,
                    },
                })
            }
            _ => Err(RestError::BadParameters(
                "Expected SVM chain_id. Ensure that the bid type matches the expected chain for the specified chain_id.".to_string()
            )),
        }
    }
}
