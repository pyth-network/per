use {
    crate::{
        api::{
            profile as ApiProfile,
            ws::{
                UpdateEvent,
                WsState,
            },
            Auth,
            RestError,
        },
        auction::{
            get_express_relay_contract,
            ChainStore,
            SignableExpressRelayContract,
        },
        config::{
            ChainId,
            ConfigEvm,
            ConfigSvm,
        },
        kernel::entities::{
            PermissionKey,
            PermissionKeySvm,
        },
        models,
        opportunity::service as opportunity_service,
        traced_client::TracedClient,
        traced_sender_svm::TracedSenderSvm,
    },
    anyhow::anyhow,
    axum::Json,
    axum_prometheus::metrics_exporter_prometheus::PrometheusHandle,
    base64::{
        engine::general_purpose::URL_SAFE_NO_PAD,
        Engine,
    },
    ethers::{
        core::k256::ecdsa::SigningKey,
        middleware::Middleware,
        prelude::{
            BlockNumber,
            Wallet,
        },
        providers::Provider,
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
    serde_json::json,
    serde_with::{
        serde_as,
        DisplayFromStr,
    },
    solana_client::nonblocking::rpc_client::RpcClient,
    solana_rpc_client::rpc_client::RpcClientConfig,
    solana_sdk::{
        commitment_config::CommitmentConfig,
        hash::Hash,
        pubkey::Pubkey,
        signature::{
            Keypair,
            Signature,
        },
        transaction::VersionedTransaction,
    },
    sqlx::{
        postgres::PgArguments,
        query::Query,
        types::{
            time::{
                OffsetDateTime,
                PrimitiveDateTime,
            },
            BigDecimal,
        },
        Postgres,
        QueryBuilder,
    },
    std::{
        collections::HashMap,
        default::Default,
        num::ParseIntError,
        ops::Deref,
        str::FromStr,
        sync::Arc,
        time::Duration,
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

pub type BidAmount = U256;
pub type BidAmountSvm = u64;
pub type GetOrCreate<T> = (T, bool);

#[derive(Clone, Debug, ToSchema, Serialize, Deserialize)]
pub struct SimulatedBidCoreFields {
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
#[schema(title = "BidResponseSvm")]
pub struct SimulatedBidSvm {
    #[serde(flatten)]
    #[schema(inline)]
    pub core_fields:    SimulatedBidCoreFields,
    /// The latest status for bid.
    pub status:         BidStatusSvm,
    /// The transaction of the bid.
    #[schema(example = "SGVsbG8sIFdvcmxkIQ==", value_type = String)]
    #[serde(with = "crate::serde::transaction_svm")]
    pub transaction:    VersionedTransaction,
    /// Amount of bid in lamports.
    #[schema(example = "1000", value_type = u64)]
    pub bid_amount:     BidAmountSvm,
    /// The permission key for bid in base64 format.
    /// This is the concatenation of the permission account and the router account.
    #[schema(example = "DUcTi3rDyS5QEmZ4BNRBejtArmDCWaPYGfN44vBJXKL5", value_type = String)]
    pub permission_key: PermissionKeySvm,
}

#[derive(Clone, Debug, ToSchema, Serialize, Deserialize)]
#[schema(title = "BidResponseEvm")]
pub struct SimulatedBidEvm {
    #[serde(flatten)]
    #[schema(inline)]
    pub core_fields:     SimulatedBidCoreFields,
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
    pub bid_amount:      BidAmount,
    /// The permission key for bid.
    #[schema(example = "0xdeadbeef", value_type = String)]
    pub permission_key:  PermissionKey,
}

// TODO - we should delete this enum and use the SimulatedBidTrait instead. We may need it for API.
#[derive(Clone, Debug, ToSchema, Serialize, Deserialize)]
#[serde(untagged)]
#[allow(clippy::large_enum_variant)]
pub enum SimulatedBid {
    Evm(SimulatedBidEvm),
    Svm(SimulatedBidSvm),
}

pub trait SimulatedBidTrait:
    Clone
    + Into<SimulatedBid>
    + std::fmt::Debug
    + TryFrom<(models::Bid, Option<models::Auction>)>
    + Deref<Target = SimulatedBidCoreFields>
    + Send
    + Sync
{
    type StatusType: BidStatusTrait;

    fn update_status(self, status: Self::StatusType) -> Self;
    fn get_metadata(&self) -> anyhow::Result<models::BidMetadata>;
    fn get_chain_type(&self) -> models::ChainType;
    fn get_bid_amount_string(&self) -> String;
    fn get_status(&self) -> &Self::StatusType;
    fn get_permission_key(&self) -> &[u8];
    fn get_permission_key_as_bytes(&self) -> Bytes {
        Bytes::from(self.get_permission_key().to_vec())
    }
    fn get_bid_status(
        status: models::BidStatus,
        index: Option<u32>,
        result: Option<<Self::StatusType as BidStatusTrait>::TxHash>,
    ) -> anyhow::Result<Self::StatusType>;
}

impl SimulatedBidTrait for SimulatedBidEvm {
    type StatusType = BidStatusEvm;

    fn update_status(self, status: Self::StatusType) -> Self {
        Self { status, ..self }
    }

    fn get_status(&self) -> &Self::StatusType {
        &self.status
    }

    fn get_permission_key(&self) -> &[u8] {
        &self.permission_key
    }

    fn get_bid_amount_string(&self) -> String {
        self.bid_amount.to_string()
    }

    fn get_metadata(&self) -> anyhow::Result<models::BidMetadata> {
        Ok(models::BidMetadata::Evm(models::BidMetadataEvm {
            target_contract: self.target_contract,
            target_calldata: self.target_calldata.clone(),
            gas_limit:       self
                .gas_limit
                .try_into()
                .map_err(|e: &str| anyhow::anyhow!(e))?,
            bundle_index:    models::BundleIndex(match self.status {
                BidStatusEvm::Pending => None,
                BidStatusEvm::Lost { index, .. } => index,
                BidStatusEvm::Submitted { index, .. } => Some(index),
                BidStatusEvm::Won { index, .. } => Some(index),
            }),
        }))
    }

    fn get_chain_type(&self) -> models::ChainType {
        models::ChainType::Evm
    }

    fn get_bid_status(
        status: models::BidStatus,
        index: Option<u32>,
        result: Option<<Self::StatusType as BidStatusTrait>::TxHash>,
    ) -> anyhow::Result<Self::StatusType> {
        match status {
            models::BidStatus::Pending => Ok(BidStatusEvm::Pending),
            models::BidStatus::Submitted => {
                if result.is_none() || index.is_none() {
                    return Err(anyhow::anyhow!(
                        "Submitted bid should have a result and index"
                    ));
                }
                Ok(BidStatusEvm::Submitted {
                    result: result.unwrap(),
                    index:  index.unwrap(),
                })
            }
            models::BidStatus::Won => {
                if result.is_none() || index.is_none() {
                    return Err(anyhow::anyhow!("Won bid should have a result and index"));
                }
                Ok(BidStatusEvm::Won {
                    result: result.unwrap(),
                    index:  index.unwrap(),
                })
            }
            models::BidStatus::Lost => Ok(BidStatusEvm::Lost { result, index }),
            models::BidStatus::Expired => Err(anyhow::anyhow!("Evm bid cannot be expired")),
        }
    }
}

impl TryFrom<(models::Bid, Option<models::Auction>)> for SimulatedBidEvm {
    type Error = anyhow::Error;

    fn try_from(
        (bid, auction): (models::Bid, Option<models::Auction>),
    ) -> Result<Self, Self::Error> {
        let core_fields = SimulatedBidCoreFields::try_from((bid.clone(), auction.clone()))?;
        let status = BidStatusEvm::extract_by(bid.clone(), auction)?;
        let metadata: models::BidMetadataEvm = bid.metadata.0.try_into()?;
        let bid_amount = BidAmount::from_dec_str(bid.bid_amount.to_string().as_str())
            .map_err(|e| anyhow::anyhow!(e))?;
        Ok(SimulatedBidEvm {
            core_fields,
            target_contract: metadata.target_contract,
            target_calldata: metadata.target_calldata,
            gas_limit: U256::from(metadata.gas_limit),
            bid_amount,
            status,
            permission_key: Bytes::from(bid.permission_key),
        })
    }
}

impl TryFrom<(models::Bid, Option<models::Auction>)> for SimulatedBidCoreFields {
    type Error = anyhow::Error;

    fn try_from(
        (bid, auction): (models::Bid, Option<models::Auction>),
    ) -> Result<Self, Self::Error> {
        if !bid.is_for_auction(&auction) {
            return Err(anyhow::anyhow!("Bid is not for the given auction"));
        }

        Ok(SimulatedBidCoreFields {
            id:              bid.id,
            chain_id:        bid.chain_id,
            initiation_time: bid.initiation_time.assume_offset(UtcOffset::UTC),
            profile_id:      bid.profile_id,
        })
    }
}


impl TryFrom<(models::Bid, Option<models::Auction>)> for SimulatedBidSvm {
    type Error = anyhow::Error;

    fn try_from(
        (bid, auction): (models::Bid, Option<models::Auction>),
    ) -> Result<Self, Self::Error> {
        let core_fields = SimulatedBidCoreFields::try_from((bid.clone(), auction.clone()))?;
        let status = BidStatusSvm::extract_by(bid.clone(), auction)?;
        let metadata: models::BidMetadataSvm = bid.metadata.0.try_into()?;
        let bid_amount = bid
            .bid_amount
            .to_string()
            .parse()
            .map_err(|e: ParseIntError| anyhow::anyhow!(e))?;
        let mut permission_key: [u8; 64] = [0; 64];
        if bid.permission_key.len() != 64 {
            return Err(anyhow::anyhow!("Invalid permission key length"));
        }
        permission_key.copy_from_slice(&bid.permission_key);
        Ok(SimulatedBidSvm {
            core_fields,
            status,
            transaction: metadata.transaction,
            bid_amount,
            permission_key: PermissionKeySvm(permission_key),
        })
    }
}

impl TryInto<(models::BidMetadata, models::ChainType)> for SimulatedBidSvm {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<(models::BidMetadata, models::ChainType), Self::Error> {
        Ok((
            models::BidMetadata::Svm(models::BidMetadataSvm {
                transaction: self.transaction,
            }),
            models::ChainType::Svm,
        ))
    }
}

impl Deref for SimulatedBidEvm {
    type Target = SimulatedBidCoreFields;

    fn deref(&self) -> &Self::Target {
        &self.core_fields
    }
}

impl Deref for SimulatedBidSvm {
    type Target = SimulatedBidCoreFields;

    fn deref(&self) -> &Self::Target {
        &self.core_fields
    }
}

impl SimulatedBidTrait for SimulatedBidSvm {
    type StatusType = BidStatusSvm;

    fn update_status(self, status: Self::StatusType) -> Self {
        Self { status, ..self }
    }

    fn get_status(&self) -> &Self::StatusType {
        &self.status
    }

    fn get_permission_key(&self) -> &[u8] {
        &self.permission_key.0
    }

    fn get_bid_amount_string(&self) -> String {
        self.bid_amount.to_string()
    }

    fn get_metadata(&self) -> anyhow::Result<models::BidMetadata> {
        Ok(models::BidMetadata::Svm(models::BidMetadataSvm {
            transaction: self.transaction.clone(),
        }))
    }

    fn get_chain_type(&self) -> models::ChainType {
        models::ChainType::Svm
    }

    fn get_bid_status(
        status: models::BidStatus,
        _index: Option<u32>,
        result: Option<<Self::StatusType as BidStatusTrait>::TxHash>,
    ) -> anyhow::Result<Self::StatusType> {
        match status {
            models::BidStatus::Pending => Ok(BidStatusSvm::Pending),
            models::BidStatus::Submitted => match result {
                Some(result) => Ok(BidStatusSvm::Submitted { result }),
                None => Err(anyhow::anyhow!("Submitted bid should have a result")),
            },
            models::BidStatus::Won => match result {
                Some(result) => Ok(BidStatusSvm::Won { result }),
                None => Err(anyhow::anyhow!("Won bid should have a result")),
            },
            models::BidStatus::Lost => Ok(BidStatusSvm::Lost { result }),
            models::BidStatus::Expired => match result {
                Some(result) => Ok(BidStatusSvm::Expired { result }),
                None => Err(anyhow::anyhow!("Expired bid should have a result")),
            },
        }
    }
}

pub type UnixTimestampMicros = i128;
pub type AuctionLock = Arc<Mutex<()>>;

pub struct ChainStoreCoreFields<T: SimulatedBidTrait> {
    pub bids:               RwLock<HashMap<PermissionKey, Vec<T>>>,
    pub auction_lock:       Mutex<HashMap<PermissionKey, AuctionLock>>,
    pub submitted_auctions: RwLock<Vec<models::Auction>>,
}

impl<T: SimulatedBidTrait> Default for ChainStoreCoreFields<T> {
    fn default() -> Self {
        Self {
            bids:               Default::default(),
            auction_lock:       Default::default(),
            submitted_auctions: Default::default(),
        }
    }
}

pub struct ChainStoreEvm {
    pub core_fields:            ChainStoreCoreFields<SimulatedBidEvm>,
    pub provider:               Provider<TracedClient>,
    pub network_id:             u64,
    // TODO move this to core fields
    pub config:                 ConfigEvm,
    pub express_relay_contract: Arc<SignableExpressRelayContract>,
    pub block_gas_limit:        U256,
    pub name:                   String,
}

impl ChainStoreEvm {
    pub fn get_chain_provider(
        chain_id: &String,
        chain_config: &ConfigEvm,
    ) -> anyhow::Result<Provider<TracedClient>> {
        let mut provider = TracedClient::new(
            chain_id.clone(),
            &chain_config.geth_rpc_addr,
            chain_config.rpc_timeout,
        )
        .map_err(|err| {
            tracing::error!(
                "Failed to create provider for chain({chain_id}) at {rpc_addr}: {:?}",
                err,
                chain_id = chain_id,
                rpc_addr = chain_config.geth_rpc_addr
            );
            anyhow!(
                "Failed to connect to chain({chain_id}) at {rpc_addr}: {:?}",
                err,
                chain_id = chain_id,
                rpc_addr = chain_config.geth_rpc_addr
            )
        })?;
        provider.set_interval(Duration::from_secs(chain_config.poll_interval));
        Ok(provider)
    }
    pub async fn create_store(
        chain_id: String,
        config: ConfigEvm,
        relayer: Wallet<SigningKey>,
    ) -> anyhow::Result<Self> {
        let provider = Self::get_chain_provider(&chain_id, &config)?;

        let id = provider.get_chainid().await?.as_u64();
        let block = provider
            .get_block(BlockNumber::Latest)
            .await?
            .expect("Failed to get latest block");

        let express_relay_contract = get_express_relay_contract(
            config.express_relay_contract,
            provider.clone(),
            relayer.clone(),
            config.legacy_tx,
            id,
        );

        Ok(Self {
            name: chain_id.clone(),
            core_fields: Default::default(),
            provider,
            network_id: id,
            config: config.clone(),
            express_relay_contract: Arc::new(express_relay_contract),
            block_gas_limit: block.gas_limit,
        })
    }
}

pub struct ChainStoreSvm {
    pub core_fields: ChainStoreCoreFields<SimulatedBidSvm>,

    tx_broadcaster_client:             RpcClient,
    pub client:                        RpcClient,
    pub config:                        ConfigSvm,
    pub express_relay_svm:             ExpressRelaySvm,
    pub wallet_program_router_account: Pubkey,
    pub name:                          String,
    pub lookup_table_cache:            LookupTableCache,
}

impl ChainStoreSvm {
    pub fn new(chain_id: String, config: ConfigSvm, relayer: Arc<Keypair>) -> Self {
        let express_relay_svm = ExpressRelaySvm {
            relayer:                     relayer.clone(),
            permission_account_position: env!("SUBMIT_BID_PERMISSION_ACCOUNT_POSITION")
                .parse::<usize>()
                .expect("Failed to parse permission account position"),
            router_account_position:     env!("SUBMIT_BID_ROUTER_ACCOUNT_POSITION")
                .parse::<usize>()
                .expect("Failed to parse router account position"),
        };

        Self {
            name: chain_id.clone(),
            core_fields: Default::default(),
            client: TracedSenderSvm::new_client(
                chain_id.clone(),
                config.rpc_read_url.as_str(),
                config.rpc_timeout,
                RpcClientConfig::with_commitment(CommitmentConfig::processed()),
            ),

            tx_broadcaster_client: TracedSenderSvm::new_client(
                chain_id.clone(),
                config.rpc_tx_submission_url.as_str(),
                config.rpc_timeout,
                RpcClientConfig::with_commitment(CommitmentConfig::processed()),
            ),

            wallet_program_router_account: config.wallet_program_router_account,
            config,
            express_relay_svm,
            lookup_table_cache: Default::default(),
        }
    }

    pub async fn send_transaction(
        &self,
        tx: &VersionedTransaction,
    ) -> solana_client::client_error::Result<Signature> {
        self.tx_broadcaster_client.send_transaction(tx).await
    }
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
    #[schema(title = "Pending")]
    Pending,
    /// The bid is submitted to the chain, with the transaction with the signature.
    /// This state is temporary and will be updated to either lost or won after conclusion of the auction.
    #[schema(title = "Submitted")]
    Submitted {
        #[schema(example = "Jb2urXPyEh4xiBgzYvwEFe4q1iMxG1DNxWGGQg94AmKgqFTwLAiTiHrYiYxwHUB4DV8u5ahNEVtMMDm3sNSRdTg", value_type = String)]
        #[serde_as(as = "DisplayFromStr")]
        result: Signature,
    },
    /// The bid lost the auction.
    /// The result will be None if the auction was concluded off-chain and no auction was submitted to the chain.
    /// The result will be not None if another bid were selected for submission to the chain.
    /// The signature of the transaction for the submitted bid is the result value.
    #[schema(title = "Lost")]
    Lost {
        #[schema(example = "Jb2urXPyEh4xiBgzYvwEFe4q1iMxG1DNxWGGQg94AmKgqFTwLAiTiHrYiYxwHUB4DV8u5ahNEVtMMDm3sNSRdTg", value_type = Option<String>)]
        #[serde(with = "crate::serde::nullable_signature_svm")]
        result: Option<Signature>,
    },
    /// The bid won the auction, with the transaction with the signature.
    #[schema(title = "Won")]
    Won {
        #[schema(example = "Jb2urXPyEh4xiBgzYvwEFe4q1iMxG1DNxWGGQg94AmKgqFTwLAiTiHrYiYxwHUB4DV8u5ahNEVtMMDm3sNSRdTg", value_type = String)]
        #[serde_as(as = "DisplayFromStr")]
        result: Signature,
    },
    /// The bid expired without being submitted on chain.
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

impl From<BidStatusEvm> for models::BidStatus {
    fn from(status: BidStatusEvm) -> Self {
        match status {
            BidStatusEvm::Pending => models::BidStatus::Pending,
            BidStatusEvm::Submitted { .. } => models::BidStatus::Submitted,
            BidStatusEvm::Lost { .. } => models::BidStatus::Lost,
            BidStatusEvm::Won { .. } => models::BidStatus::Won,
        }
    }
}

impl From<BidStatusSvm> for models::BidStatus {
    fn from(status: BidStatusSvm) -> Self {
        match status {
            BidStatusSvm::Pending => models::BidStatus::Pending,
            BidStatusSvm::Submitted { .. } => models::BidStatus::Submitted,
            BidStatusSvm::Lost { .. } => models::BidStatus::Lost,
            BidStatusSvm::Won { .. } => models::BidStatus::Won,
            BidStatusSvm::Expired { .. } => models::BidStatus::Expired,
        }
    }
}

impl From<BidStatus> for models::BidStatus {
    fn from(status: BidStatus) -> Self {
        match status {
            BidStatus::Evm(status) => status.into(),
            BidStatus::Svm(status) => status.into(),
        }
    }
}

pub trait BidStatusTrait:
    Clone
    + std::fmt::Debug
    + PartialEq<models::BidStatus>
    + Into<BidStatus>
    + Into<models::BidStatus>
    + Send
    + Sync
{
    type TxHash: Clone + std::fmt::Debug + AsRef<[u8]> + Send + Sync;

    fn get_update_query(
        &self,
        id: BidId,
        auction: Option<&models::Auction>,
    ) -> anyhow::Result<Query<'_, Postgres, PgArguments>>;
    fn extract_by(bid: models::Bid, auction: Option<models::Auction>) -> anyhow::Result<Self>;
    fn convert_tx_hash(tx_hash: &Self::TxHash) -> Vec<u8> {
        tx_hash.as_ref().to_vec()
    }
    fn get_tx_hash(&self) -> Option<&Self::TxHash>;
}

impl From<BidStatusEvm> for BidStatus {
    fn from(bid_status: BidStatusEvm) -> BidStatus {
        BidStatus::Evm(bid_status)
    }
}

impl From<BidStatusSvm> for BidStatus {
    fn from(bid_status: BidStatusSvm) -> BidStatus {
        BidStatus::Svm(bid_status)
    }
}

impl PartialEq<models::BidStatus> for BidStatusEvm {
    fn eq(&self, other: &models::BidStatus) -> bool {
        *other == self.clone().into()
    }
}

impl PartialEq<models::BidStatus> for BidStatusSvm {
    fn eq(&self, other: &models::BidStatus) -> bool {
        *other == self.clone().into()
    }
}

impl BidStatusTrait for BidStatusEvm {
    type TxHash = H256;

    fn get_update_query(
        &self,
        id: BidId,
        auction: Option<&models::Auction>,
    ) -> anyhow::Result<Query<'_, Postgres, PgArguments>> {
        match self {
            BidStatusEvm::Pending => Err(anyhow::anyhow!("Cannot update pending bid status")),
            BidStatusEvm::Submitted { index, .. } => {
                match auction {
                    Some(auction) =>
                        Ok(sqlx::query!(
                            "UPDATE bid SET status = $1, auction_id = $2, metadata = jsonb_set(metadata, '{bundle_index}', $3) WHERE id = $4 AND status = $5",
                            models::BidStatus::Submitted as _,
                            auction.id,
                            json!(index),
                            id,
                            models::BidStatus::Pending as _,
                        )),
                    None => Err(anyhow::anyhow!(
                        "Cannot update submitted bid status without auction."
                    )),
                }
            }
            BidStatusEvm::Lost { index, .. } => {
                match auction {
                    Some(auction) => {
                        match index {
                            Some(index) => {
                                Ok(sqlx::query!(
                                    "UPDATE bid SET status = $1, metadata = jsonb_set(metadata, '{bundle_index}', $2), auction_id = $3 WHERE id = $4 AND status = $5",
                                    models::BidStatus::Lost as _,
                                    json!(index),
                                    auction.id,
                                    id,
                                    models::BidStatus::Submitted as _
                                ))
                            },
                            None => Ok(sqlx::query!(
                                "UPDATE bid SET status = $1, auction_id = $2 WHERE id = $3 AND status = $4",
                                models::BidStatus::Lost as _,
                                auction.id,
                                id,
                                models::BidStatus::Pending as _,
                            )),
                        }
                    },
                    None => Ok(sqlx::query!(
                        "UPDATE bid SET status = $1 WHERE id = $2 AND status = $3",
                        models::BidStatus::Lost as _,
                        id,
                        models::BidStatus::Pending as _
                    )),
                }
            },
            BidStatusEvm::Won { index, .. } => Ok(sqlx::query!(
                "UPDATE bid SET status = $1, metadata = jsonb_set(metadata, '{bundle_index}', $2) WHERE id = $3 AND status = $4",
                models::BidStatus::Won as _,
                json!(index),
                id,
                models::BidStatus::Submitted as _,
            )),
        }
    }

    fn extract_by(bid: models::Bid, auction: Option<models::Auction>) -> anyhow::Result<Self> {
        if !bid.is_for_auction(&auction) {
            return Err(anyhow::anyhow!("Bid is not for the given auction"));
        }
        if bid.status == models::BidStatus::Pending {
            Ok(BidStatusEvm::Pending)
        } else {
            let result = match auction {
                Some(auction) => auction.tx_hash.map(|tx_hash| H256::from_slice(&tx_hash)),
                None => None,
            };
            let index = bid.metadata.0.get_bundle_index();
            if bid.status == models::BidStatus::Lost {
                Ok(BidStatusEvm::Lost { result, index })
            } else {
                if result.is_none() || index.is_none() {
                    return Err(anyhow::anyhow!(
                        "Won or submitted bid must have a transaction hash and index"
                    ));
                }
                let result = result.expect("Invalid result for won or submitted bid");
                let index = index.expect("Invalid index for won or submitted bid");
                if bid.status == models::BidStatus::Won {
                    Ok(BidStatusEvm::Won { result, index })
                } else if bid.status == models::BidStatus::Submitted {
                    Ok(BidStatusEvm::Submitted { result, index })
                } else {
                    Err(anyhow::anyhow!("Invalid bid status".to_string()))
                }
            }
        }
    }

    fn get_tx_hash(&self) -> Option<&Self::TxHash> {
        match self {
            BidStatusEvm::Submitted { result, .. } => Some(result),
            BidStatusEvm::Lost { result, .. } => result.as_ref(),
            BidStatusEvm::Won { result, .. } => Some(result),
            _ => None,
        }
    }
}

impl BidStatusTrait for BidStatusSvm {
    type TxHash = Signature;

    fn get_update_query(
        &self,
        id: BidId,
        auction: Option<&models::Auction>,
    ) -> anyhow::Result<Query<'_, Postgres, PgArguments>> {
        match self {
            BidStatusSvm::Pending => Err(anyhow::anyhow!("Cannot update pending bid status")),
            BidStatusSvm::Submitted { .. } => match auction {
                Some(auction) => Ok(sqlx::query!(
                    "UPDATE bid SET status = $1, auction_id = $2 WHERE id = $3 AND status = $4",
                    models::BidStatus::Submitted as _,
                    auction.id,
                    id,
                    models::BidStatus::Pending as _,
                )),
                None => Err(anyhow::anyhow!(
                    "Cannot update submitted bid status without auction."
                )),
            },
            BidStatusSvm::Lost { .. } => match auction {
                Some(auction) => Ok(sqlx::query!(
                    "UPDATE bid SET status = $1, auction_id = $2 WHERE id = $3 AND status IN ($4, $5)",
                    models::BidStatus::Lost as _,
                    auction.id,
                    id,
                    models::BidStatus::Pending as _,
                    models::BidStatus::Submitted as _,
                )),
                None => Ok(sqlx::query!(
                    "UPDATE bid SET status = $1 WHERE id = $2 AND status = $3",
                    models::BidStatus::Lost as _,
                    id,
                    models::BidStatus::Pending as _
                )),
            },
            BidStatusSvm::Won { .. } => Ok(sqlx::query!(
                "UPDATE bid SET status = $1 WHERE id = $2 AND status = $3",
                models::BidStatus::Won as _,
                id,
                models::BidStatus::Submitted as _,
            )),
            BidStatusSvm::Expired { .. } => Ok(sqlx::query!(
                "UPDATE bid SET status = $1 WHERE id = $2 AND status = $3",
                models::BidStatus::Expired as _,
                id,
                models::BidStatus::Submitted as _,
            )),
        }
    }

    fn extract_by(bid: models::Bid, auction: Option<models::Auction>) -> anyhow::Result<Self> {
        if !bid.is_for_auction(&auction) {
            return Err(anyhow::anyhow!("Bid is not for the given auction"));
        }
        if bid.status == models::BidStatus::Pending {
            Ok(BidStatusSvm::Pending)
        } else {
            let result = match auction {
                Some(auction) => match auction.tx_hash {
                    Some(tx_hash) => Some(
                        Signature::try_from(tx_hash)
                            .map_err(|_| anyhow::anyhow!("Error reading signature"))?,
                    ),
                    None => None,
                },
                None => None,
            };
            if bid.status == models::BidStatus::Lost {
                Ok(BidStatusSvm::Lost { result })
            } else {
                if result.is_none() {
                    return Err(anyhow::anyhow!(
                        "Won or submitted bid must have a transaction hash and index"
                    ));
                }
                let result = result.expect("Invalid result for won or submitted bid");
                if bid.status == models::BidStatus::Won {
                    Ok(BidStatusSvm::Won { result })
                } else if bid.status == models::BidStatus::Submitted {
                    Ok(BidStatusSvm::Submitted { result })
                } else {
                    Err(anyhow::anyhow!("Invalid bid status".to_string()))
                }
            }
        }
    }

    fn get_tx_hash(&self) -> Option<&Self::TxHash> {
        match self {
            BidStatusSvm::Submitted { result } => Some(result),
            BidStatusSvm::Lost { result } => result.as_ref(),
            BidStatusSvm::Won { result } => Some(result),
            _ => None,
        }
    }
}

#[derive(Serialize, Clone, ToSchema, ToResponse)]
pub struct BidStatusWithId {
    #[schema(value_type = String)]
    pub id:         BidId,
    pub bid_status: BidStatus,
}

#[derive(Clone)]
pub struct ExpressRelaySvm {
    pub relayer:                     Arc<Keypair>,
    pub permission_account_position: usize,
    pub router_account_position:     usize,
}

pub type LookupTableCache = RwLock<HashMap<Pubkey, Vec<Pubkey>>>;

pub struct Store {
    pub chains_evm:       HashMap<ChainId, Arc<ChainStoreEvm>>,
    pub chains_svm:       HashMap<ChainId, Arc<ChainStoreSvm>>,
    pub event_sender:     broadcast::Sender<UpdateEvent>,
    pub ws:               WsState,
    pub db:               sqlx::PgPool,
    pub task_tracker:     TaskTracker,
    pub secret_key:       String,
    pub access_tokens:    RwLock<HashMap<models::AccessTokenToken, models::Profile>>,
    pub metrics_recorder: PrometheusHandle,
}

pub struct StoreNew {
    pub opportunity_service_evm:
        Arc<opportunity_service::Service<opportunity_service::ChainTypeEvm>>,
    pub opportunity_service_svm:
        Arc<opportunity_service::Service<opportunity_service::ChainTypeSvm>>,
    pub store:                   Arc<Store>,
}

impl SimulatedBidCoreFields {
    pub fn new(chain_id: String, initiation_time: OffsetDateTime, auth: Auth) -> Self {
        Self {
            id: Uuid::new_v4(),
            chain_id,
            initiation_time,
            profile_id: match auth {
                Auth::Authorized(_, profile) => Some(profile.id),
                _ => None,
            },
        }
    }
}

impl TryFrom<(models::Bid, Option<models::Auction>)> for BidStatus {
    type Error = anyhow::Error;

    fn try_from(
        (bid, auction): (models::Bid, Option<models::Auction>),
    ) -> Result<Self, Self::Error> {
        match bid.metadata.0 {
            models::BidMetadata::Evm(_) => {
                Ok(BidStatus::Evm(BidStatusEvm::extract_by(bid, auction)?))
            }
            models::BidMetadata::Svm(_) => {
                Ok(BidStatus::Svm(BidStatusSvm::extract_by(bid, auction)?))
            }
        }
    }
}

impl From<SimulatedBidEvm> for SimulatedBid {
    fn from(bid: SimulatedBidEvm) -> Self {
        SimulatedBid::Evm(bid)
    }
}

impl From<SimulatedBidSvm> for SimulatedBid {
    fn from(bid: SimulatedBidSvm) -> Self {
        SimulatedBid::Svm(bid)
    }
}

impl Store {
    #[tracing::instrument(skip_all)]
    pub async fn init_auction<T: ChainStore>(
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
            chain_type: T::CHAIN_TYPE,
            tx_hash: None,
            bid_collection_time: Some(PrimitiveDateTime::new(
                bid_collection_time.date(),
                bid_collection_time.time(),
            )),
            submission_time: None,
        };
        sqlx::query!(
            "INSERT INTO auction (id, creation_time, permission_key, chain_id, chain_type, bid_collection_time) VALUES ($1, $2, $3, $4, $5, $6)",
            auction.id,
            auction.creation_time,
            auction.permission_key,
            auction.chain_id,
            auction.chain_type as _,
            auction.bid_collection_time,
        )
        .execute(&self.db)
        .await?;
        Ok(auction)
    }

    #[tracing::instrument(skip_all)]
    pub async fn submit_auction<T: ChainStore>(
        &self,
        chain_store: &T,
        mut auction: models::Auction,
        transaction_hash: Vec<u8>,
    ) -> anyhow::Result<models::Auction> {
        auction.tx_hash = Some(transaction_hash);
        let now = OffsetDateTime::now_utc();
        auction.submission_time = Some(PrimitiveDateTime::new(now.date(), now.time()));
        sqlx::query!("UPDATE auction SET submission_time = $1, tx_hash = $2 WHERE id = $3 AND submission_time IS NULL",
            auction.submission_time,
            auction.tx_hash,
            auction.id)
            .execute(&self.db)
            .await?;

        chain_store.add_submitted_auction(auction.clone()).await;
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
            auction.id,
        )
        .execute(&self.db)
        .await?;
        Ok(auction)
    }

    #[tracing::instrument(skip_all)]
    pub async fn add_bid<T: ChainStore>(
        &self,
        chain_store: &T,
        bid: T::SimulatedBid,
    ) -> Result<(), RestError> {
        let now = OffsetDateTime::now_utc();

        let metadata = bid.get_metadata().map_err(|e: anyhow::Error| {
            tracing::error!("Failed to get metadata: {}", e);
            RestError::TemporarilyUnavailable
        })?;
        let chain_type = bid.get_chain_type();
        let status: models::BidStatus = bid.get_status().clone().into();

        sqlx::query!("INSERT INTO bid (id, creation_time, permission_key, chain_id, chain_type, bid_amount, status, initiation_time, profile_id, metadata) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
        bid.id,
        PrimitiveDateTime::new(now.date(), now.time()),
        bid.get_permission_key().to_vec(),
        bid.chain_id,
        chain_type as _,
        BigDecimal::from_str(&bid.get_bid_amount_string()).unwrap(),
        status as _,
        PrimitiveDateTime::new(bid.initiation_time.date(), bid.initiation_time.time()),
        bid.profile_id,
        serde_json::to_value(metadata).expect("Failed to serialize metadata"))
            .execute(&self.db)
            .await.map_err(|e| {
            tracing::error!("DB: Failed to insert bid: {}", e);
            RestError::TemporarilyUnavailable
        })?;

        chain_store.add_bid(bid.clone()).await;

        self.broadcast_status_update(BidStatusWithId {
            id:         bid.id,
            bid_status: bid.get_status().clone().into(),
        });
        Ok(())
    }

    pub async fn get_bid_status(&self, bid_id: BidId) -> Result<Json<BidStatus>, RestError> {
        // TODO handle it in a single query (Maybe with intermediate type)
        let bid: models::Bid = sqlx::query_as("SELECT * FROM bid WHERE id = $1")
            .bind(bid_id)
            .fetch_one(&self.db)
            .await
            .map_err(|e| {
                tracing::warn!("DB: Failed to get bid: {} - bid_id: {}", e, bid_id);
                RestError::BidNotFound
            })?;

        let auction = match bid.auction_id {
            Some(auction_id) => {
                let auction: models::Auction =
                    sqlx::query_as("SELECT * FROM auction WHERE id = $1")
                        .bind(auction_id)
                        .fetch_one(&self.db)
                        .await
                        .map_err(|e| {
                            tracing::warn!(
                                "DB: Failed to get auction: {} - auction_id: {}",
                                e,
                                auction_id
                            );
                            RestError::TemporarilyUnavailable
                        })?;
                Some(auction)
            }
            None => None,
        };

        let bid_status: BidStatus = (bid, auction).try_into().map_err(|e: anyhow::Error| {
            tracing::warn!("Failed to convert bid status: {}", e);
            RestError::TemporarilyUnavailable
        })?;
        Ok(Json(bid_status))
    }

    pub async fn broadcast_bid_status_and_update<T: ChainStore>(
        &self,
        chain_store: &T,
        bid: T::SimulatedBid,
        updated_status: <T::SimulatedBid as SimulatedBidTrait>::StatusType,
        auction: Option<&models::Auction>,
    ) -> anyhow::Result<()> {
        let status: models::BidStatus = updated_status.clone().into();
        let bid_id = bid.id;
        let update_query = updated_status.get_update_query(bid_id, auction)?;
        let query_result = update_query.execute(&self.db).await?;
        match status {
            models::BidStatus::Pending => {}
            models::BidStatus::Submitted => {
                let updated_bid = bid.update_status(updated_status.clone());
                chain_store.update_bid(updated_bid).await;
            }
            models::BidStatus::Lost => chain_store.remove_bid(bid.clone()).await,
            models::BidStatus::Won => chain_store.remove_bid(bid.clone()).await,
            models::BidStatus::Expired => chain_store.remove_bid(bid.clone()).await,
        }

        // It is possible to call this function multiple times from different threads if receipts are delayed
        // Or the new block is mined faster than the bid status is updated.
        // To ensure we do not broadcast the update more than once, we need to check the below "if"
        if query_result.rows_affected() > 0 {
            self.broadcast_status_update(BidStatusWithId {
                id:         bid_id,
                bid_status: updated_status.into(),
            });
        }
        Ok(())
    }

    fn broadcast_status_update(&self, update: BidStatusWithId) {
        if let Err(e) = self.event_sender.send(UpdateEvent::BidStatusUpdate(update)) {
            tracing::error!("Failed to send bid status update: {}", e)
        };
    }

    pub fn broadcast_svm_chain_update(&self, update: SvmChainUpdate) {
        if let Err(e) = self.event_sender.send(UpdateEvent::SvmChainUpdate(update)) {
            tracing::error!("Failed to send chain update: {}", e)
        };
    }

    pub async fn create_profile(
        &self,
        create_profile: ApiProfile::CreateProfile,
    ) -> Result<models::Profile, RestError> {
        let id = Uuid::new_v4();
        let role: models::ProfileRole = create_profile.role.clone().into();
        let profile: models::Profile = sqlx::query_as(
            "INSERT INTO profile (id, name, email, role) VALUES ($1, $2, $3, $4) RETURNING id, name, email, role, created_at, updated_at",
        ).bind(id)
        .bind(create_profile.name.clone())
        .bind(create_profile.email.to_string())
        .bind(role)
        .fetch_one(&self.db).await
        .map_err(|e| {
            if let Some(true) = e.as_database_error().map(|e| e.is_unique_violation()) {
                return RestError::BadParameters("Profile with this email already exists".to_string());
            }
            tracing::error!("DB: Failed to insert profile: {} - profile_data: {:?}", e, create_profile);
            RestError::TemporarilyUnavailable
        })?;
        Ok(profile)
    }

    fn generate_url_safe_token(&self) -> anyhow::Result<String> {
        let mut rng = rand::thread_rng();
        let bytes: [u8; 32] = rng.gen();
        Ok(URL_SAFE_NO_PAD.encode(bytes))
    }

    pub async fn get_profile_by_email(
        &self,
        email: models::EmailAddress,
    ) -> Result<Option<models::Profile>, RestError> {
        sqlx::query_as("SELECT * FROM profile WHERE email = $1")
            .bind(email.0.to_string())
            .fetch_optional(&self.db)
            .await
            .map_err(|e| {
                tracing::error!("DB: Failed to fetch profile: {} - email: {}", e, email.0);
                RestError::TemporarilyUnavailable
            })
    }

    pub async fn get_profile_by_id(
        &self,
        id: models::ProfileId,
    ) -> Result<Option<models::Profile>, RestError> {
        sqlx::query_as("SELECT * FROM profile WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.db)
            .await
            .map_err(|e| {
                tracing::error!("DB: Failed to fetch profile: {} - id: {}", e, id);
                RestError::TemporarilyUnavailable
            })
    }

    pub async fn get_or_create_access_token(
        &self,
        profile_id: models::ProfileId,
    ) -> Result<GetOrCreate<models::AccessToken>, RestError> {
        let generated_token = self.generate_url_safe_token().map_err(|e| {
            tracing::error!(
                "Failed to generate access token: {} - profile_id: {}",
                e,
                profile_id
            );
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
            tracing::error!(
                "DB: Failed to create access token: {} - profile_id: {}",
                e,
                profile_id
            );
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
            tracing::error!(
                "DB: Failed to fetch access token: {} - profile_id: {}",
                e,
                profile_id
            );
            RestError::TemporarilyUnavailable
        })?;

        let profile = self
            .get_profile_by_id(profile_id)
            .await?
            .ok_or_else(|| RestError::BadParameters("Profile id not found".to_string()))?;
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
        let mut query = QueryBuilder::new("SELECT * from bid where profile_id = ");
        query.push_bind(profile_id);
        if let Some(from_time) = from_time {
            query.push(" AND initiation_time >= ");
            query.push_bind(from_time);
        }
        query.push(" ORDER BY initiation_time ASC LIMIT 20");
        query
            .build_query_as()
            .fetch_all(&self.db)
            .await
            .map_err(|e| {
                tracing::error!("DB: Failed to fetch bids: {}", e);
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
                let result: anyhow::Result<SimulatedBid> = match b.chain_type {
                    models::ChainType::Evm => {
                        let bid: anyhow::Result<SimulatedBidEvm> =
                            (b.clone(), auction.clone()).try_into();
                        bid.map(|b| b.into())
                    }
                    models::ChainType::Svm => {
                        let bid: anyhow::Result<SimulatedBidSvm> =
                            (b.clone(), auction.clone()).try_into();
                        bid.map(|b| b.into())
                    }
                };
                match result {
                    Ok(bid) => Some(bid),
                    Err(e) => {
                        tracing::error!(
                            "Failed to convert bid to SimulatedBid: {} - bid: {:?}",
                            e,
                            b
                        );
                        None
                    }
                }
            })
            .collect())
    }
}

#[serde_as]
#[derive(Serialize, Clone, ToSchema, ToResponse)]
pub struct SvmChainUpdate {
    pub chain_id:  ChainId,
    #[serde_as(as = "DisplayFromStr")]
    pub blockhash: Hash,
}
