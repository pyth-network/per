use {
    crate::{
        api::ws::{
            UpdateEvent,
            WsState,
        },
        config::{
            ChainId,
            EthereumConfig,
        },
    },
    dashmap::DashMap,
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
    std::collections::{
        HashMap,
        HashSet,
    },
    tokio::sync::{
        broadcast,
        RwLock,
    },
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
    pub id:         BidId,
    pub contract:   Address,
    pub calldata:   Bytes,
    pub bid_amount: BidAmount,
    // simulation_time:
}

pub type UnixTimestamp = i64;

#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq)]
pub struct TokenAmount {
    /// Token contract address
    #[schema(example = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",value_type=String)]
    pub token:  ethers::abi::Address,
    /// Token amount
    #[schema(example = "1000", value_type=String)]
    #[serde(with = "crate::serde::u256")]
    pub amount: U256,
}

/// Opportunity parameters needed for on-chain execution
/// If a searcher signs the opportunity and have approved enough tokens to opportunity adapter,
/// by calling this contract with the given calldata and structures, they will receive the tokens specified
/// in the buy_tokens field, and will send the tokens specified in the sell_tokens field.
#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq)]
pub struct OpportunityParamsV1 {
    /// The permission key required for successful execution of the opportunity.
    #[schema(example = "0xdeadbeefcafe", value_type=String)]
    pub permission_key: Bytes,
    /// The chain id where the opportunity will be executed.
    #[schema(example = "sepolia", value_type=String)]
    pub chain_id:       ChainId,
    /// The contract address to call for execution of the opportunity.
    #[schema(example = "0xcA11bde05977b3631167028862bE2a173976CA11", value_type=String)]
    pub contract:       ethers::abi::Address,
    /// Calldata for the contract call.
    #[schema(example = "0xdeadbeef", value_type=String)]
    pub calldata:       Bytes,
    /// The value to send with the contract call.
    #[schema(example = "1", value_type=String)]
    #[serde(with = "crate::serde::u256")]
    pub value:          U256,

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
    pub creation_time: UnixTimestamp,
    pub params:        OpportunityParams,
    pub bidders:       HashSet<Address>,
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
    pub provider:         Provider<Http>,
    pub network_id:       u64,
    pub config:           EthereumConfig,
    pub token_spoof_info: RwLock<HashMap<Address, SpoofInfo>>,
    pub bids:             RwLock<HashMap<PermissionKey, Vec<SimulatedBid>>>,
}

#[derive(Default)]
pub struct OpportunityStore {
    pub opportunities: DashMap<PermissionKey, Vec<Opportunity>>,
}

pub type BidId = Uuid;

#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq)]
#[serde(tag = "status", content = "result", rename_all = "snake_case")]
pub enum BidStatus {
    /// The auction for this bid is pending
    Pending,
    /// The bid won the auction and was submitted to the chain in a transaction with the given hash
    #[schema(example = "0x103d4fbd777a36311b5161f2062490f761f25b67406badb2bace62bb170aa4e3", value_type=String)]
    Submitted(H256),
    /// The bid lost the auction
    Lost,
}

#[derive(Serialize, Clone, ToSchema, ToResponse)]
pub struct BidStatusWithId {
    #[schema(value_type = String)]
    pub id:         BidId,
    pub bid_status: BidStatus,
}

pub struct BidStatusStore {
    pub bids_status:  RwLock<HashMap<BidId, BidStatus>>,
    pub event_sender: broadcast::Sender<UpdateEvent>,
}

impl BidStatusStore {
    pub async fn get_status(&self, id: &BidId) -> Option<BidStatus> {
        self.bids_status.read().await.get(id).cloned()
    }

    pub async fn set_and_broadcast(&self, update: BidStatusWithId) {
        self.bids_status
            .write()
            .await
            .insert(update.id, update.bid_status.clone());
        match self.event_sender.send(UpdateEvent::BidStatusUpdate(update)) {
            Ok(_) => (),
            Err(e) => tracing::error!("Failed to send bid status update: {}", e),
        };
    }
}

pub struct Store {
    pub chains:            HashMap<ChainId, ChainStore>,
    pub bid_status_store:  BidStatusStore,
    pub opportunity_store: OpportunityStore,
    pub relayer:           LocalWallet,
    pub ws:                WsState,
}
