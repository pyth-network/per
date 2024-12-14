use {
    super::{
        entities::{
            self,
            BidChainData,
        },
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
            Auth,
            RestError,
            WrappedRouter,
        },
        kernel::entities::{
            ChainId,
            Evm,
            Svm,
        },
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
        Json,
        Router,
    },
    express_relay_api_types::{
        bid::{
            Bid,
            BidCoreFields,
            BidCreate,
            BidCreateEvm,
            BidCreateSvm,
            BidEvm,
            BidId,
            BidResult,
            BidStatus,
            BidStatusEvm,
            BidStatusSvm,
            BidSvm,
            Bids,
            GetBidStatusParams,
            GetBidsByTimeQueryParams,
            Route,
        },
        ErrorBodyResponse,
    },
    sqlx::types::time::OffsetDateTime,
    std::{
        fmt::Debug,
        sync::Arc,
    },
};

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
    WrappedRouter::new(store)
        .route(Route::PostBid, post_bid)
        .route(Route::GetBidsByTime, get_bids_by_time)
        .route(Route::GetBidStatus, get_bid_status)
        .route(
            express_relay_api_types::bid::DeprecatedRoute::DeprecatedGetBidsByTime,
            get_bids_by_time_deprecated,
        )
        .route(
            express_relay_api_types::bid::DeprecatedRoute::DeprecatedGetBidStatus,
            get_bid_status_deprecated,
        )
        .router
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

fn get_core_fields<T: ChainTrait>(bid: &entities::Bid<T>) -> BidCoreFields {
    BidCoreFields {
        id:              bid.id,
        chain_id:        bid.chain_id.clone(),
        initiation_time: bid.initiation_time,
        profile_id:      bid.profile_id,
    }
}

impl From<entities::Bid<Evm>> for Bid {
    fn from(bid: entities::Bid<Evm>) -> Self {
        Bid::Evm(BidEvm {
            core_fields:     get_core_fields(&bid),
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
            core_fields:    get_core_fields(&bid),
            permission_key: express_relay_api_types::PermissionKeySvm(
                bid.chain_data.get_permission_key().0,
            ),
            status:         bid.status.into(),
            transaction:    bid.chain_data.transaction,
            bid_amount:     bid.amount,
        })
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
                    },
                })
            }
            _ => Err(RestError::BadParameters(
                "Expected SVM chain_id. Ensure that the bid type matches the expected chain for the specified chain_id.".to_string()
            )),
        }
    }
}
