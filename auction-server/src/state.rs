use {
    crate::config::{
        ChainId,
        EthereumConfig,
    },
    ethers::{
        providers::{
            Http,
            Provider,
        },
        signers::LocalWallet,
        types::{
            Address,
            Bytes,
            U256,
        },
    },
    std::collections::{
        HashMap,
        HashSet,
    },
    tokio::sync::RwLock,
    uuid::Uuid,
};

pub type PermissionKey = Bytes;

#[derive(Clone)]
pub struct SimulatedBid {
    pub contract: Address,
    pub calldata: Bytes,
    pub bid:      U256,
    // simulation_time:
}

pub type UnixTimestamp = i64;
#[derive(Clone)]
pub struct VerifiedLiquidationOpportunity {
    pub id:             Uuid,
    pub creation_time:  UnixTimestamp,
    pub chain_id:       ChainId,
    pub permission_key: PermissionKey,
    pub contract:       Address,
    pub calldata:       Bytes,
    pub value:          U256,
    pub repay_tokens:   Vec<(Address, U256)>,
    pub receipt_tokens: Vec<(Address, U256)>,
    pub bidders:        HashSet<Address>,
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
pub struct LiquidationStore {
    pub opportunities: RwLock<HashMap<PermissionKey, Vec<VerifiedLiquidationOpportunity>>>,
}

pub struct Store {
    pub chains:            HashMap<ChainId, ChainStore>,
    pub liquidation_store: LiquidationStore,
    pub per_operator:      LocalWallet,
}
