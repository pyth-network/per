use {
    crate::{
        api::{
            opportunity::EIP712Domain,
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
    },
    axum::Json,
    ethers::{
        providers::{
            Http,
            Provider,
        },
        signers::LocalWallet,
        types::{
            Address,
            Bytes,
            H256,
            U256,
        },
    },
    serde::{
        Deserialize,
        Serialize,
    },
    sqlx::{
        database::HasArguments,
        encode::IsNull,
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
        collections::HashMap,
        str::FromStr,
        sync::Arc,
        time::Duration,
    },
    tokio::sync::{
        broadcast,
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

#[derive(Clone)]
pub struct SimulatedBid {
    pub id:              BidId,
    pub target_contract: Address,
    pub target_calldata: Bytes,
    pub bid_amount:      BidAmount,
    pub permission_key:  PermissionKey,
    pub chain_id:        ChainId,
    pub status:          BidStatus,
    pub initiation_time: OffsetDateTime,
}

pub type UnixTimestampMicros = i128;

#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq)]
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
    pub provider:               Provider<Http>,
    pub network_id:             u64,
    pub config:                 EthereumConfig,
    pub weth:                   Address,
    pub token_spoof_info:       RwLock<HashMap<Address, SpoofInfo>>,
    pub eip_712_domain:         EIP712Domain,
    pub express_relay_contract: Arc<SignableExpressRelayContract>,
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

#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BidStatus {
    /// The auction for this bid is pending
    Pending,
    /// The bid simulation was failed in the final simulation step before submission
    SimulationFailed,
    /// The bid won the auction, which concluded with it being placed in the index position of the multicall at the given hash
    Submitted {
        #[schema(example = "0x103d4fbd777a36311b5161f2062490f761f25b67406badb2bace62bb170aa4e3", value_type = String)]
        result: H256,
        #[schema(example = 1, value_type = u32)]
        index:  u32,
    },
    /// The bid lost the auction, which concluded with the transaction with the given hash
    Lost {
        #[schema(example = "0x103d4fbd777a36311b5161f2062490f761f25b67406badb2bace62bb170aa4e3", value_type = String)]
        result: H256,
    },
}

impl sqlx::Encode<'_, sqlx::Postgres> for BidStatus {
    fn encode_by_ref(&self, buf: &mut <Postgres as HasArguments<'_>>::ArgumentBuffer) -> IsNull {
        let result = match self {
            BidStatus::Pending => "pending",
            BidStatus::SimulationFailed => "simulation_failed",
            BidStatus::Submitted {
                result: _,
                index: _,
            } => "submitted",
            BidStatus::Lost { result: _ } => "lost",
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
    pub chains:               HashMap<ChainId, ChainStore>,
    pub bids:                 RwLock<HashMap<BidId, SimulatedBid>>,
    pub event_sender:         broadcast::Sender<UpdateEvent>,
    pub opportunity_store:    OpportunityStore,
    pub relayer:              LocalWallet,
    pub ws:                   WsState,
    pub db:                   sqlx::PgPool,
    pub task_tracker:         TaskTracker,
    pub in_progress_auctions: RwLock<HashMap<(PermissionKey, ChainId), OffsetDateTime>>,
}

const AUCTION_LOCK_DURATION: Duration = Duration::from_secs(30);

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
            tx_hash: None,
            bid_collection_time: Some(PrimitiveDateTime::new(
                bid_collection_time.date(),
                bid_collection_time.time(),
            )),
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

    pub async fn conclude_auction(
        &self,
        mut auction: models::Auction,
        transaction_hash: H256,
    ) -> anyhow::Result<models::Auction> {
        auction.tx_hash = Some(transaction_hash.as_bytes().to_vec());
        let now = OffsetDateTime::now_utc();
        auction.conclusion_time = Some(PrimitiveDateTime::new(now.date(), now.time()));
        sqlx::query!("UPDATE auction SET conclusion_time = $1, tx_hash = $2 WHERE id = $3 AND conclusion_time IS NULL",
            auction.conclusion_time,
            auction.tx_hash,
            auction.id)
            .execute(&self.db)
            .await?;
        Ok(auction)
    }

    pub async fn add_bid(&self, bid: SimulatedBid) -> Result<(), RestError> {
        let bid_id = bid.id;
        let now = OffsetDateTime::now_utc();
        sqlx::query!("INSERT INTO bid (id, creation_time, permission_key, chain_id, target_contract, target_calldata, bid_amount, status, initiation_time) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
        bid.id,
        PrimitiveDateTime::new(now.date(), now.time()),
        bid.permission_key.to_vec(),
        bid.chain_id,
        &bid.target_contract.to_fixed_bytes(),
        bid.target_calldata.to_vec(),
        BigDecimal::from_str(&bid.bid_amount.to_string()).unwrap(),
        bid.status as _,
        PrimitiveDateTime::new(bid.initiation_time.date(), bid.initiation_time.time()),
        )
            .execute(&self.db)
            .await.map_err(|e| {
            tracing::error!("DB: Failed to insert bid: {}", e);
            RestError::TemporarilyUnavailable
        })?;

        self.bids.write().await.insert(bid_id, bid.clone());
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
                } else if status == "simulation_failed" {
                    status_json = BidStatus::SimulationFailed.into();
                } else {
                    match status_data.tx_hash {
                        Some(tx_hash) => {
                            let tx_hash = H256::from_slice(&tx_hash);
                            if status == "lost" {
                                status_json = BidStatus::Lost { result: tx_hash }.into();
                            } else if status == "submitted" {
                                match status_data.bundle_index {
                                    Some(bundle_index) => {
                                        status_json = BidStatus::Submitted {
                                            result: tx_hash,
                                            index:  bundle_index as u32,
                                        }
                                        .into();
                                    }
                                    None => {
                                        return Err(RestError::BadParameters(
                                            "Submitted bid must have bundle index".to_string(),
                                        ));
                                    }
                                }
                            } else {
                                return Err(RestError::BadParameters(
                                    "Invalid bid status".to_string(),
                                ));
                            }
                        }
                        None => {
                            return Err(RestError::BadParameters(
                                "Lost or submitted bid must have a transaction hash".to_string(),
                            ));
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

    pub async fn broadcast_bid_status_and_remove(
        &self,
        update: BidStatusWithId,
        auction: Option<&models::Auction>,
    ) -> anyhow::Result<()> {
        match update.bid_status {
            BidStatus::Pending => {
                return Err(anyhow::anyhow!(
                    "Bid status cannot remain pending when removing a bid."
                ));
            }
            BidStatus::SimulationFailed => {
                sqlx::query!(
                    "UPDATE bid SET status = $1 WHERE id = $2 AND auction_id is NULL",
                    update.bid_status as _,
                    update.id
                )
                .execute(&self.db)
                .await?;
            }
            BidStatus::Submitted { result: _, index } => {
                if let Some(auction) = auction {
                    sqlx::query!(
                        "UPDATE bid SET status = $1, auction_id = $2, bundle_index = $3 WHERE id = $4 AND auction_id is NULL",
                        update.bid_status as _,
                        auction.id,
                        index as i32,
                        update.id
                    )
                    .execute(&self.db)
                    .await?;
                } else {
                    return Err(anyhow::anyhow!(
                        "Cannot broadcast submitted bid status without auction."
                    ));
                }
            }
            BidStatus::Lost { result: _ } => {
                if let Some(auction) = auction {
                    sqlx::query!(
                        "UPDATE bid SET status = $1, auction_id = $2 WHERE id = $3 AND auction_id is NULL",
                        update.bid_status as _,
                        auction.id,
                        update.id
                    )
                    .execute(&self.db)
                    .await?;
                } else {
                    return Err(anyhow::anyhow!(
                        "Cannot broadcast lost bid status without auction."
                    ));
                }
            }
        }

        self.bids.write().await.remove(&update.id);
        self.broadcast_status_update(update);
        Ok(())
    }

    fn broadcast_status_update(&self, update: BidStatusWithId) {
        match self.event_sender.send(UpdateEvent::BidStatusUpdate(update)) {
            Ok(_) => (),
            Err(e) => tracing::error!("Failed to send bid status update: {}", e),
        };
    }

    pub async fn get_bids_by_chain_id(&self, chain_id: &ChainId) -> Vec<SimulatedBid> {
        self.bids
            .read()
            .await
            .values()
            .filter(|bid| bid.chain_id.eq(chain_id))
            .cloned()
            .collect()
    }

    // Returns true if auction time was updated, false otherwise
    pub async fn update_in_progress_auctions(
        &self,
        permission_key: PermissionKey,
        chain_id: ChainId,
    ) -> bool {
        let now = OffsetDateTime::now_utc();
        let mut in_progress_auctions = self.in_progress_auctions.write().await;
        if let Some(existing_auction_time) =
            in_progress_auctions.get(&(permission_key.clone(), chain_id.clone()))
        {
            if *existing_auction_time > now - AUCTION_LOCK_DURATION {
                return false;
            }
        }
        in_progress_auctions.insert((permission_key, chain_id), now);
        true
    }

    pub async fn remove_in_progress_auction(
        &self,
        permission_key: PermissionKey,
        chain_id: ChainId,
    ) {
        self.in_progress_auctions
            .write()
            .await
            .remove(&(permission_key, chain_id));
    }
}
