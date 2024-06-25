use {
    crate::{
        api::{
            opportunity::{
                EIP712Domain,
                OpportunityParamsWithMetadata,
            },
            profile as ApiProfile,
            ws::{
                UpdateEvent,
                WsState,
            },
            RestError,
        },
        auction::SignableExpressRelayContract,
        config::{
            ChainId,
            EthereumConfig,
        },
        models,
        traced_client::TracedClient,
    },
    axum::Json,
    axum_prometheus::metrics_exporter_prometheus::PrometheusHandle,
    base64::{
        engine::general_purpose::URL_SAFE_NO_PAD,
        Engine,
    },
    ethers::{
        providers::Provider,
        signers::LocalWallet,
        types::{
            Address,
            Bytes,
            H256,
            U256,
        },
    },
    rand::Rng,
    serde::{
        Deserialize,
        Serialize,
    },
    sqlx::{
        database::HasArguments,
        encode::IsNull,
        postgres::PgQueryResult,
        types::{
            time::{
                OffsetDateTime,
                PrimitiveDateTime,
            },
            BigDecimal,
        },
        Postgres,
        TypeInfo,
    },
    std::{
        collections::{
            hash_map::Entry,
            HashMap,
        },
        str::FromStr,
        sync::Arc,
    },
    time::UtcOffset,
    tokio::sync::{
        broadcast,
        Mutex,
        RwLock,
    },
    tokio_util::task::TaskTracker,
    utoipa::{
        ToResponse,
        ToSchema,
    },
    uuid::Uuid,
};

pub type PermissionKey = Bytes;
pub type BidAmount = U256;
pub type GetOrCreate<T> = (T, bool);

#[derive(Clone, Debug, ToSchema, Serialize, Deserialize)]
#[schema(title = "BidResponse")]
pub struct SimulatedBid {
    /// The unique id for bid.
    #[schema(example = "obo3ee3e-58cc-4372-a567-0e02b2c3d479", value_type = String)]
    pub id:              BidId,
    /// The contract address to call.
    #[schema(example = "0xcA11bde05977b3631167028862bE2a173976CA11", value_type = String)]
    pub target_contract: Address,
    /// Calldata for the contract call.
    #[schema(example = "0xdeadbeef", value_type = String)]
    pub target_calldata: Bytes,
    /// Amount of bid in wei.
    #[schema(example = "10", value_type = String)]
    #[serde(with = "crate::serde::u256")]
    pub bid_amount:      BidAmount,
    /// The permission key for bid.
    #[schema(example = "0xdeadbeef", value_type = String)]
    pub permission_key:  PermissionKey,
    /// The chain id for bid.
    #[schema(example = "op_sepolia", value_type = String)]
    pub chain_id:        ChainId,
    /// The latest status for bid.
    #[schema(example = "op_sepolia", value_type = BidStatus)]
    pub status:          BidStatus,
    /// The time server received the bid formatted in rfc3339.
    #[schema(example = "2024-05-23T21:26:57.329954Z", value_type = String)]
    #[serde(with = "time::serde::rfc3339")]
    pub initiation_time: OffsetDateTime,
    /// The profile id for the bid owner.
    #[schema(example = "", value_type = String)]
    pub profile_id:      Option<models::ProfileId>,
}

pub type UnixTimestampMicros = i128;

#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug)]
pub struct TokenAmount {
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
#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq)]
pub struct OpportunityParamsV1 {
    /// The permission key required for successful execution of the opportunity.
    #[schema(example = "0xdeadbeefcafe", value_type = String)]
    pub permission_key:    Bytes,
    /// The chain id where the opportunity will be executed.
    #[schema(example = "op_sepolia", value_type = String)]
    pub chain_id:          ChainId,
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

    pub sell_tokens: Vec<TokenAmount>,
    pub buy_tokens:  Vec<TokenAmount>,
}

#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq)]
#[serde(tag = "version")]
pub enum OpportunityParams {
    #[serde(rename = "v1")]
    V1(OpportunityParamsV1),
}

pub type OpportunityId = Uuid;
pub type AuctionKey = (PermissionKey, ChainId);
pub type AuctionLock = Arc<Mutex<()>>;

#[derive(Clone, PartialEq)]
pub struct Opportunity {
    pub id:            OpportunityId,
    pub creation_time: UnixTimestampMicros,
    pub params:        OpportunityParams,
}

#[derive(Clone)]
pub enum SpoofInfo {
    Spoofed {
        balance_slot:   U256,
        allowance_slot: U256,
    },
    UnableToSpoof,
}

pub struct ChainStore {
    pub provider:               Provider<TracedClient>,
    pub network_id:             u64,
    pub config:                 EthereumConfig,
    pub weth:                   Address,
    pub token_spoof_info:       RwLock<HashMap<Address, SpoofInfo>>,
    pub eip_712_domain:         EIP712Domain,
    pub express_relay_contract: Arc<SignableExpressRelayContract>,
    pub block_gas_limit:        U256,
}

#[derive(Default)]
pub struct OpportunityStore {
    pub opportunities: RwLock<HashMap<PermissionKey, Vec<Opportunity>>>,
}

impl OpportunityStore {
    pub async fn add_opportunity(&self, opportunity: Opportunity) {
        let key = match &opportunity.params {
            OpportunityParams::V1(params) => params.permission_key.clone(),
        };
        self.opportunities
            .write()
            .await
            .entry(key)
            .or_insert_with(Vec::new)
            .push(opportunity);
    }
}

pub type BidId = Uuid;

#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BidStatus {
    /// The temporary state which means the auction for this bid is pending
    Pending,
    /// The bid is submitted to the chain, which is placed at the given index of the transaction with the given hash
    /// This state is temporary and will be updated to either lost or won after conclusion of the auction
    Submitted {
        #[schema(example = "0x103d4fbd777a36311b5161f2062490f761f25b67406badb2bace62bb170aa4e3", value_type = String)]
        result: H256,
        #[schema(example = 1, value_type = u32)]
        index:  u32,
    },
    /// The bid lost the auction, which is concluded with the transaction with the given hash and index
    /// The result will be None if the auction was concluded off-chain and no auction was submitted to the chain
    /// The index will be None if the bid was not submitted to the chain and lost the auction by off-chain calculation
    /// There are cases where the result is not None and the index is None.
    /// It is because other bids were selected for submission to the chain, but not this one.
    Lost {
        #[schema(example = "0x103d4fbd777a36311b5161f2062490f761f25b67406badb2bace62bb170aa4e3", value_type = Option<String>)]
        result: Option<H256>,
        #[schema(example = 1, value_type = Option<u32>)]
        index:  Option<u32>,
    },
    /// The bid won the auction, which is concluded with the transaction with the given hash and index
    Won {
        #[schema(example = "0x103d4fbd777a36311b5161f2062490f761f25b67406badb2bace62bb170aa4e3", value_type = String)]
        result: H256,
        #[schema(example = 1, value_type = u32)]
        index:  u32,
    },
}

impl sqlx::Encode<'_, sqlx::Postgres> for BidStatus {
    fn encode_by_ref(&self, buf: &mut <Postgres as HasArguments<'_>>::ArgumentBuffer) -> IsNull {
        let result = match self {
            BidStatus::Pending => "pending",
            BidStatus::Submitted {
                result: _,
                index: _,
            } => "submitted",
            BidStatus::Lost {
                result: _,
                index: _,
            } => "lost",
            BidStatus::Won {
                result: _,
                index: _,
            } => "won",
        };
        <&str as sqlx::Encode<sqlx::Postgres>>::encode(result, buf)
    }
}

impl sqlx::Type<sqlx::Postgres> for BidStatus {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        sqlx::postgres::PgTypeInfo::with_name("bid_status")
    }

    fn compatible(ty: &sqlx::postgres::PgTypeInfo) -> bool {
        ty.name() == "bid_status"
    }
}

#[derive(Serialize, Clone, ToSchema, ToResponse)]
pub struct BidStatusWithId {
    #[schema(value_type = String)]
    pub id:         BidId,
    pub bid_status: BidStatus,
}

pub struct Store {
    pub chains:             HashMap<ChainId, ChainStore>,
    pub bids:               RwLock<HashMap<AuctionKey, Vec<SimulatedBid>>>,
    pub event_sender:       broadcast::Sender<UpdateEvent>,
    pub opportunity_store:  OpportunityStore,
    pub relayer:            LocalWallet,
    pub ws:                 WsState,
    pub db:                 sqlx::PgPool,
    pub task_tracker:       TaskTracker,
    pub auction_lock:       Mutex<HashMap<AuctionKey, AuctionLock>>,
    pub submitted_auctions: RwLock<HashMap<ChainId, Vec<models::Auction>>>,
    pub secret_key:         String,
    pub access_tokens:      RwLock<HashMap<models::AccessTokenToken, models::Profile>>,
    pub metrics_recorder:   PrometheusHandle,
}

impl SimulatedBid {
    pub fn get_auction_key(&self) -> AuctionKey {
        (self.permission_key.clone(), self.chain_id.clone())
    }
}

impl TryFrom<(models::Bid, Option<models::Auction>)> for BidStatus {
    type Error = anyhow::Error;

    fn try_from(
        (bid, auction): (models::Bid, Option<models::Auction>),
    ) -> Result<Self, Self::Error> {
        if !bid.is_for_auction(&auction) {
            return Err(anyhow::anyhow!("Bid is not for the given auction"));
        }
        if bid.status == models::BidStatus::Pending {
            Ok(BidStatus::Pending)
        } else {
            let result = match auction {
                Some(auction) => auction.tx_hash.0,
                None => None,
            };
            let index = bid.bundle_index;
            if bid.status == models::BidStatus::Lost {
                Ok(BidStatus::Lost {
                    result,
                    index: index.0,
                })
            } else {
                if result.is_none() || index.is_none() {
                    return Err(anyhow::anyhow!(
                        "Won or submitted bid must have a transaction hash and index"
                    ));
                }
                let result = result.unwrap();
                let index = index.unwrap();
                if bid.status == models::BidStatus::Won {
                    Ok(BidStatus::Won { result, index })
                } else if bid.status == models::BidStatus::Submitted {
                    Ok(BidStatus::Submitted { result, index })
                } else {
                    Err(anyhow::anyhow!("Invalid bid status".to_string()))
                }
            }
        }
    }
}

impl TryFrom<(models::Bid, Option<models::Auction>)> for SimulatedBid {
    type Error = anyhow::Error;

    fn try_from(
        (bid, auction): (models::Bid, Option<models::Auction>),
    ) -> Result<Self, Self::Error> {
        if !bid.is_for_auction(&auction) {
            return Err(anyhow::anyhow!("Bid is not for the given auction"));
        }
        let bid_amount = BidAmount::from_dec_str(bid.bid_amount.to_string().as_str())
            .map_err(|e| anyhow::anyhow!(e))?;
        let bid_with_auction = (bid.clone(), auction);
        Ok(SimulatedBid {
            id: bid.id,
            target_contract: Address::from_slice(&bid.target_contract),
            target_calldata: Bytes::from(bid.target_calldata),
            bid_amount,
            permission_key: Bytes::from(bid.permission_key),
            chain_id: bid.chain_id,
            status: bid_with_auction.try_into()?,
            initiation_time: bid.initiation_time.assume_offset(UtcOffset::UTC),
            profile_id: bid.profile_id,
        })
    }
}

impl Store {
    pub async fn opportunity_exists(&self, opportunity: &Opportunity) -> bool {
        let key = match &opportunity.params {
            OpportunityParams::V1(params) => params.permission_key.clone(),
        };
        self.opportunity_store
            .opportunities
            .read()
            .await
            .get(&key)
            .map_or(false, |opps| opps.contains(opportunity))
    }

    pub async fn add_opportunity(&self, opportunity: Opportunity) -> Result<(), RestError> {
        let odt = OffsetDateTime::from_unix_timestamp_nanos(opportunity.creation_time * 1000)
            .expect("creation_time is valid");
        let OpportunityParams::V1(params) = &opportunity.params;
        sqlx::query!("INSERT INTO opportunity (id,
                                                        creation_time,
                                                        permission_key,
                                                        chain_id,
                                                        target_contract,
                                                        target_call_value,
                                                        target_calldata,
                                                        sell_tokens,
                                                        buy_tokens) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
        opportunity.id,
        PrimitiveDateTime::new(odt.date(), odt.time()),
        params.permission_key.to_vec(),
        params.chain_id,
        &params.target_contract.to_fixed_bytes(),
        BigDecimal::from_str(&params.target_call_value.to_string()).unwrap(),
        params.target_calldata.to_vec(),
        serde_json::to_value(&params.sell_tokens).unwrap(),
        serde_json::to_value(&params.buy_tokens).unwrap())
            .execute(&self.db)
            .await
            .map_err(|e| {
                tracing::error!("DB: Failed to insert opportunity: {}", e);
                RestError::TemporarilyUnavailable
            })?;
        self.opportunity_store.add_opportunity(opportunity).await;
        Ok(())
    }

    pub async fn remove_opportunity(&self, opportunity: &Opportunity) -> anyhow::Result<()> {
        let key = match &opportunity.params {
            OpportunityParams::V1(params) => params.permission_key.clone(),
        };
        let mut write_guard = self.opportunity_store.opportunities.write().await;
        let entry = write_guard.entry(key.clone());
        if entry
            .and_modify(|opps| opps.retain(|o| o != opportunity))
            .or_default()
            .is_empty()
        {
            write_guard.remove(&key);
        }
        drop(write_guard);
        let now = OffsetDateTime::now_utc();
        sqlx::query!(
            "UPDATE opportunity SET removal_time = $1 WHERE id = $2 AND removal_time IS NULL",
            PrimitiveDateTime::new(now.date(), now.time()),
            opportunity.id
        )
        .execute(&self.db)
        .await?;
        Ok(())
    }

    #[tracing::instrument(skip_all)]
    pub async fn init_auction(
        &self,
        permission_key: PermissionKey,
        chain_id: ChainId,
        bid_collection_time: OffsetDateTime,
    ) -> anyhow::Result<models::Auction> {
        let now = OffsetDateTime::now_utc();
        let auction = models::Auction {
            id: Uuid::new_v4(),
            creation_time: PrimitiveDateTime::new(now.date(), now.time()),
            conclusion_time: None,
            permission_key: permission_key.to_vec(),
            chain_id,
            tx_hash: models::TxHash(None),
            bid_collection_time: Some(PrimitiveDateTime::new(
                bid_collection_time.date(),
                bid_collection_time.time(),
            )),
            submission_time: None,
        };
        sqlx::query!(
            "INSERT INTO auction (id, creation_time, permission_key, chain_id, bid_collection_time) VALUES ($1, $2, $3, $4, $5)",
            auction.id,
            auction.creation_time,
            auction.permission_key,
            auction.chain_id,
            auction.bid_collection_time,
        )
        .execute(&self.db)
        .await?;
        Ok(auction)
    }

    #[tracing::instrument(skip_all)]
    pub async fn submit_auction(
        &self,
        mut auction: models::Auction,
        transaction_hash: H256,
    ) -> anyhow::Result<models::Auction> {
        auction.tx_hash = models::TxHash(Some(transaction_hash));
        let now = OffsetDateTime::now_utc();
        auction.submission_time = Some(PrimitiveDateTime::new(now.date(), now.time()));
        sqlx::query!("UPDATE auction SET submission_time = $1, tx_hash = $2 WHERE id = $3 AND submission_time IS NULL",
            auction.submission_time,
            auction.tx_hash.map(|h| h.as_bytes().to_vec()),
            auction.id)
            .execute(&self.db)
            .await?;

        self.submitted_auctions
            .write()
            .await
            .entry(auction.clone().chain_id)
            .or_insert_with(Vec::new)
            .push(auction.clone());
        Ok(auction)
    }

    #[tracing::instrument(skip_all)]
    pub async fn conclude_auction(
        &self,
        mut auction: models::Auction,
    ) -> anyhow::Result<models::Auction> {
        let now = OffsetDateTime::now_utc();
        auction.conclusion_time = Some(PrimitiveDateTime::new(now.date(), now.time()));
        sqlx::query!(
            "UPDATE auction SET conclusion_time = $1 WHERE id = $2 AND conclusion_time IS NULL",
            auction.conclusion_time,
            auction.id
        )
        .execute(&self.db)
        .await?;
        Ok(auction)
    }

    pub async fn get_bids(&self, key: &AuctionKey) -> Vec<SimulatedBid> {
        self.bids.read().await.get(key).cloned().unwrap_or_default()
    }

    pub async fn get_permission_keys_for_auction(&self, chain_id: &ChainId) -> Vec<PermissionKey> {
        self.bids
            .read()
            .await
            .keys()
            .filter_map(|(p, c)| {
                if c != chain_id {
                    return None;
                }
                Some(p.clone())
            })
            .collect()
    }

    pub async fn get_submitted_auctions(&self, chain_id: &ChainId) -> Vec<models::Auction> {
        self.submitted_auctions
            .read()
            .await
            .get(chain_id)
            .cloned()
            .unwrap_or_default()
    }

    #[tracing::instrument(skip_all)]
    pub async fn add_bid(&self, bid: SimulatedBid) -> Result<(), RestError> {
        let bid_id = bid.id;
        let now = OffsetDateTime::now_utc();
        sqlx::query!("INSERT INTO bid (id, creation_time, permission_key, chain_id, target_contract, target_calldata, bid_amount, status, initiation_time, profile_id) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
        bid.id,
        PrimitiveDateTime::new(now.date(), now.time()),
        bid.permission_key.to_vec(),
        bid.chain_id,
        &bid.target_contract.to_fixed_bytes(),
        bid.target_calldata.to_vec(),
        BigDecimal::from_str(&bid.bid_amount.to_string()).unwrap(),
        bid.status as _,
        PrimitiveDateTime::new(bid.initiation_time.date(), bid.initiation_time.time()),
        bid.profile_id,
        )
            .execute(&self.db)
            .await.map_err(|e| {
            tracing::error!("DB: Failed to insert bid: {}", e);
            RestError::TemporarilyUnavailable
        })?;

        self.bids
            .write()
            .await
            .entry(bid.get_auction_key())
            .or_insert_with(Vec::new)
            .push(bid.clone());

        self.broadcast_status_update(BidStatusWithId {
            id:         bid_id,
            bid_status: bid.status.clone(),
        });
        Ok(())
    }

    pub async fn get_bid_status(&self, bid_id: BidId) -> Result<Json<BidStatus>, RestError> {
        let status_data = sqlx::query!(
            // TODO: improve the call here to not cast to text
            "SELECT status::text, auction_id, bundle_index, tx_hash FROM (bid LEFT OUTER JOIN auction ON bid.auction_id = auction.id) WHERE bid.id = $1",
            bid_id
        )
        .fetch_one(&self.db)
        .await
        .map_err(|_| RestError::BidNotFound)?;

        let status_json: Json<BidStatus>;
        match status_data.status {
            Some(status) => {
                if status == "pending" {
                    status_json = BidStatus::Pending.into();
                } else {
                    let result = status_data
                        .tx_hash
                        .map(|tx_hash| H256::from_slice(&tx_hash));
                    let index = status_data.bundle_index.map(|i| i as u32);
                    if status == "lost" {
                        status_json = BidStatus::Lost { result, index }.into();
                    } else {
                        if result.is_none() || index.is_none() {
                            return Err(RestError::BadParameters(
                                "Won or submitted bid must have a transaction hash and index"
                                    .to_string(),
                            ));
                        }
                        let result = result.unwrap();
                        let index = index.unwrap();
                        if status == "won" {
                            status_json = BidStatus::Won { result, index }.into();
                        } else if status == "submitted" {
                            status_json = BidStatus::Submitted { result, index }.into();
                        } else {
                            return Err(RestError::BadParameters("Invalid bid status".to_string()));
                        }
                    }
                }
            }
            None => {
                return Err(RestError::BidNotFound);
            }
        }

        Ok(status_json)
    }

    async fn remove_bid(&self, bid: SimulatedBid) {
        let mut write_guard = self.bids.write().await;
        let key = bid.get_auction_key();
        if let Entry::Occupied(mut entry) = write_guard.entry(key.clone()) {
            let bids = entry.get_mut();
            bids.retain(|b| b.id != bid.id);
            if bids.is_empty() {
                entry.remove();
            }
        }
    }

    pub async fn bids_for_submitted_auction(&self, auction: models::Auction) -> Vec<SimulatedBid> {
        let bids = self
            .get_bids(&(
                auction.permission_key.clone().into(),
                auction.chain_id.clone(),
            ))
            .await;
        match auction.tx_hash.0 {
            Some(tx_hash) => bids
                .into_iter()
                .filter(|bid| match bid.status {
                    BidStatus::Submitted { result, .. } => result == tx_hash,
                    _ => false,
                })
                .collect(),
            None => vec![],
        }
    }

    pub async fn remove_submitted_auction(&self, auction: models::Auction) {
        if !self
            .bids_for_submitted_auction(auction.clone())
            .await
            .is_empty()
        {
            return;
        }

        let mut write_guard = self.submitted_auctions.write().await;
        let key: String = auction.chain_id;
        if let Entry::Occupied(mut entry) = write_guard.entry(key) {
            let auctions = entry.get_mut();
            auctions.retain(|a| a.id != auction.id);
            if auctions.is_empty() {
                entry.remove();
            }
        }
    }

    async fn update_bid(&self, bid: SimulatedBid) {
        let mut write_guard = self.bids.write().await;
        let key = bid.get_auction_key();
        match write_guard.entry(key.clone()) {
            Entry::Occupied(mut entry) => {
                let bids = entry.get_mut();
                match bids.iter().position(|b| b.id == bid.id) {
                    Some(index) => bids[index] = bid,
                    None => {
                        tracing::error!("Update bid failed - bid not found for: {:?}", bid);
                    }
                }
            }
            Entry::Vacant(_) => {
                tracing::error!("Update bid failed - entry not found for key: {:?}", key);
            }
        }
    }

    pub async fn broadcast_bid_status_and_update(
        &self,
        bid: SimulatedBid,
        updated_status: BidStatus,
        auction: Option<&models::Auction>,
    ) -> anyhow::Result<()> {
        let query_result: PgQueryResult;
        match updated_status {
            BidStatus::Pending => {
                return Err(anyhow::anyhow!(
                    "Bid status cannot remain pending when removing a bid."
                ));
            }
            BidStatus::Submitted { result: _, index } => {
                if let Some(auction) = auction {
                    query_result = sqlx::query!(
                        "UPDATE bid SET status = $1, auction_id = $2, bundle_index = $3 WHERE id = $4 AND status = 'pending'",
                        updated_status as _,
                        auction.id,
                        index as i32,
                        bid.id
                    )
                    .execute(&self.db)
                    .await?;

                    let mut submitted_bid = bid.clone();
                    submitted_bid.status = updated_status.clone();
                    self.update_bid(submitted_bid).await;
                } else {
                    return Err(anyhow::anyhow!(
                        "Cannot broadcast submitted bid status without auction."
                    ));
                }
            }
            BidStatus::Lost { result: _, index } => {
                if let Some(auction) = auction {
                    query_result = sqlx::query!(
                        "UPDATE bid SET status = $1, bundle_index = $2, auction_id = $3 WHERE id = $4 AND status = 'submitted'",
                        updated_status as _,
                        index.map(|i| i as i32),
                        auction.id,
                        bid.id
                    )
                    .execute(&self.db)
                    .await?;
                } else {
                    query_result = sqlx::query!(
                        "UPDATE bid SET status = $1, bundle_index = $2 WHERE id = $3 AND status = 'pending'",
                        updated_status as _,
                        index.map(|i| i as i32),
                        bid.id
                    )
                    .execute(&self.db)
                    .await?;
                }
                self.remove_bid(bid.clone()).await;
            }
            BidStatus::Won { result: _, index } => {
                query_result = sqlx::query!(
                    "UPDATE bid SET status = $1, bundle_index = $2 WHERE id = $3 AND status = 'submitted'",
                    updated_status as _,
                    index as i32,
                    bid.id
                )
                .execute(&self.db)
                .await?;
                self.remove_bid(bid.clone()).await;
            }
        }

        // It is possible to call this function multiple times from different threads if receipts are delayed
        // Or the new block is mined faster than the bid status is updated.
        // To ensure we do not broadcast the update more than once, we need to check the below "if"
        if query_result.rows_affected() > 0 {
            self.broadcast_status_update(BidStatusWithId {
                id:         bid.id,
                bid_status: updated_status,
            });
        }
        Ok(())
    }

    fn broadcast_status_update(&self, update: BidStatusWithId) {
        match self.event_sender.send(UpdateEvent::BidStatusUpdate(update)) {
            Ok(_) => (),
            Err(e) => tracing::error!("Failed to send bid status update: {}", e),
        };
    }

    pub async fn get_auction_lock(&self, key: AuctionKey) -> AuctionLock {
        self.auction_lock
            .lock()
            .await
            .entry(key)
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    pub async fn remove_auction_lock(&self, key: &AuctionKey) {
        let mut mutex_gaurd = self.auction_lock.lock().await;
        let auction_lock = mutex_gaurd.get(key);
        if let Some(auction_lock) = auction_lock {
            // Whenever there is no thread borrowing a lock for this key, we can remove it from the locks HashMap.
            if Arc::strong_count(auction_lock) == 1 {
                mutex_gaurd.remove(key);
            }
        }
    }

    pub async fn create_profile(
        &self,
        create_profile: ApiProfile::CreateProfile,
    ) -> Result<models::Profile, RestError> {
        let id = Uuid::new_v4();
        let profile: models::Profile = sqlx::query_as(
            "INSERT INTO profile (id, name, email) VALUES ($1, $2, $3) RETURNING id, name, email, created_at, updated_at",
        ).bind(id)
        .bind(create_profile.name)
        .bind(create_profile.email.to_string()).fetch_one(&self.db).await
        .map_err(|e| {
            if let Some(true) = e.as_database_error().map(|e| e.is_unique_violation()) {
                return RestError::BadParameters("Profile with this email already exists".to_string());
            }
            tracing::error!("DB: Failed to insert profile: {}", e);
            RestError::TemporarilyUnavailable
        })?;
        Ok(profile)
    }

    fn generate_url_safe_token(&self) -> anyhow::Result<String> {
        let mut rng = rand::thread_rng();
        let bytes: [u8; 32] = rng.gen();
        Ok(URL_SAFE_NO_PAD.encode(bytes))
    }

    pub async fn get_profile_by_id(
        &self,
        id: models::ProfileId,
    ) -> Result<models::Profile, RestError> {
        sqlx::query_as("SELECT * FROM profile WHERE id = $1")
            .bind(id)
            .fetch_one(&self.db)
            .await
            .map_err(|e| {
                tracing::error!("DB: Failed to fetch profile: {}", e);
                RestError::TemporarilyUnavailable
            })
    }

    pub async fn get_or_create_access_token(
        &self,
        profile_id: models::ProfileId,
    ) -> Result<GetOrCreate<models::AccessToken>, RestError> {
        let generated_token = self.generate_url_safe_token().map_err(|e| {
            tracing::error!("Failed to generate access token: {}", e);
            RestError::TemporarilyUnavailable
        })?;

        let id = Uuid::new_v4();
        let result = sqlx::query!(
            "INSERT INTO access_token (id, profile_id, token)
        SELECT $1, $2, $3
        WHERE NOT EXISTS (
            SELECT id
            FROM access_token
            WHERE profile_id = $2 AND revoked_at is NULL
        );",
            id,
            profile_id,
            generated_token
        )
        .execute(&self.db)
        .await
        .map_err(|e| {
            tracing::error!("DB: Failed to create access token: {}", e);
            RestError::TemporarilyUnavailable
        })?;

        let token = sqlx::query_as!(
            models::AccessToken,
            "SELECT * FROM access_token
        WHERE profile_id = $1 AND revoked_at is NULL;",
            profile_id,
        )
        .fetch_one(&self.db)
        .await
        .map_err(|e| {
            tracing::error!("DB: Failed to fetch access token: {}", e);
            RestError::TemporarilyUnavailable
        })?;

        let profile = self.get_profile_by_id(profile_id).await?;
        self.access_tokens
            .write()
            .await
            .insert(token.token.clone(), profile);
        Ok((token, result.rows_affected() > 0))
    }

    pub async fn revoke_access_token(
        &self,
        token: &models::AccessTokenToken,
    ) -> Result<(), RestError> {
        sqlx::query!(
            "UPDATE access_token
        SET revoked_at = now()
        WHERE token = $1 AND revoked_at is NULL;",
            token
        )
        .execute(&self.db)
        .await
        .map_err(|e| {
            tracing::error!("DB: Failed to revoke access token: {}", e);
            RestError::TemporarilyUnavailable
        })?;

        self.access_tokens.write().await.remove(token);
        Ok(())
    }

    pub async fn get_profile_by_token(
        &self,
        token: &models::AccessTokenToken,
    ) -> Result<models::Profile, RestError> {
        self.access_tokens
            .read()
            .await
            .get(token)
            .cloned()
            .ok_or(RestError::InvalidToken)
    }

    async fn get_bids_by_time(
        &self,
        profile_id: models::ProfileId,
        from_time: Option<OffsetDateTime>,
    ) -> Result<Vec<models::Bid>, RestError> {
        let select = "SELECT * FROM bid WHERE profile_id = $1";
        let order_by = "ORDER BY initiation_time ASC LIMIT 20";
        let query_with_time = format!("{} AND initiation_time >= $2 {}", select, order_by);
        let query_without_time = format!("{} {}", select, order_by);

        let query = match from_time {
            Some(from_time) => sqlx::query_as(query_with_time.as_str())
                .bind(profile_id)
                .bind(from_time),
            None => sqlx::query_as(query_without_time.as_str()).bind(profile_id),
        };
        query.fetch_all(&self.db).await.map_err(|e| {
            tracing::error!("DB: Failed to fetch bids: {}", e);
            RestError::TemporarilyUnavailable
        })
    }

    pub async fn get_opportunities_by_permission_key(
        &self,
        chain_id: ChainId,
        permission_key: PermissionKey,
        from_time: Option<OffsetDateTime>,
    ) -> Result<Vec<OpportunityParamsWithMetadata>, RestError> {
        let chain_store = self
            .chains
            .get(&chain_id)
            .ok_or_else(|| RestError::InvalidChainId)?;
        let select = "SELECT * FROM opportunity WHERE chain_id = $1 AND permission_key = $2";
        let order_by = "ORDER BY creation_time DESC LIMIT 20";
        let query_with_time = format!("{} AND creation_time >= $3 {}", select, order_by);
        let query_without_time = format!("{} {}", select, order_by);

        let query = match from_time {
            Some(from_time) => sqlx::query_as(query_with_time.as_str())
                .bind(&chain_id)
                .bind(permission_key.to_vec())
                .bind(from_time),
            None => sqlx::query_as(query_without_time.as_str())
                .bind(&chain_id)
                .bind(permission_key.to_vec()),
        };
        let opps: Vec<models::Opportunity> = query.fetch_all(&self.db).await.map_err(|e| {
            tracing::error!("DB: Failed to fetch opportunities: {}", e);
            RestError::TemporarilyUnavailable
        })?;

        let parsed_opps: anyhow::Result<Vec<OpportunityParamsWithMetadata>> = opps
            .into_iter()
            .map(|opp| {
                let params: OpportunityParams = OpportunityParams::V1(OpportunityParamsV1 {
                    permission_key:    Bytes::from(opp.permission_key.clone()),
                    chain_id:          opp.chain_id,
                    target_contract:   ethers::abi::Address::from_slice(&opp.target_contract),
                    target_calldata:   Bytes::from(opp.target_calldata),
                    target_call_value: U256::from_dec_str(
                        opp.target_call_value.to_string().as_str(),
                    )?,
                    sell_tokens:       serde_json::from_value(opp.sell_tokens)?,
                    buy_tokens:        serde_json::from_value(opp.buy_tokens)?,
                });
                let opp = Opportunity {
                    id: opp.id,
                    creation_time: opp.creation_time.assume_utc().unix_timestamp_nanos(),
                    params,
                };
                Ok(OpportunityParamsWithMetadata::from(opp, chain_store))
            })
            .collect();
        parsed_opps.map_err(|e| {
            tracing::error!(
                "Failed to convert opportunity to OpportunityParamsWithMetadata: {}",
                e
            );
            RestError::TemporarilyUnavailable
        })
    }

    async fn get_auctions_by_bids(
        &self,
        bids: &[models::Bid],
    ) -> Result<Vec<models::Auction>, RestError> {
        let auction_ids: Vec<models::AuctionId> =
            bids.iter().filter_map(|bid| bid.auction_id).collect();
        sqlx::query_as("SELECT * FROM auction WHERE id = ANY($1)")
            .bind(auction_ids)
            .fetch_all(&self.db)
            .await
            .map_err(|e| {
                tracing::error!("DB: Failed to fetch auctions: {}", e);
                RestError::TemporarilyUnavailable
            })
    }

    pub async fn get_simulated_bids_by_time(
        &self,
        profile_id: models::ProfileId,
        from_time: Option<OffsetDateTime>,
    ) -> Result<Vec<SimulatedBid>, RestError> {
        let bids = self.get_bids_by_time(profile_id, from_time).await?;
        let auctions = self.get_auctions_by_bids(&bids).await?;

        Ok(bids
            .into_iter()
            .filter_map(|b| {
                let auction = match b.auction_id {
                    Some(auction_id) => auctions.clone().into_iter().find(|a| a.id == auction_id),
                    None => None,
                };
                let result: anyhow::Result<SimulatedBid> = (b, auction).try_into();
                match result {
                    Ok(bid) => Some(bid),
                    Err(e) => {
                        tracing::error!("Failed to convert bid to SimulatedBid: {}", e);
                        None
                    }
                }
            })
            .collect())
    }
}
