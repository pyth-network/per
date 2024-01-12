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
    std::collections::HashMap,
    tokio::sync::RwLock,
    uuid::Uuid,
};

pub type PermissionKey = Bytes;
pub type OrderId = Uuid;

#[derive(Clone)]
pub struct SimulatedBid {
    pub contract: Address,
    pub calldata: Bytes,
    pub bid:      U256,
    // simulation_time:
}

#[derive(Clone)]
pub struct VerifiedOrder {
    pub chain:          ChainId,
    pub permission:     PermissionKey,
    pub contract:       Address,
    pub calldata:       Bytes,
    pub repay_tokens:   Vec<(Address, U256)>,
    pub receipt_tokens: Vec<(Address, U256)>,
}
pub struct ChainStore {
    pub provider:   Provider<Http>,
    pub network_id: u64,
    pub config:     EthereumConfig,
    pub bids:       RwLock<HashMap<PermissionKey, Vec<SimulatedBid>>>,
}


#[derive(Default)]
pub struct LiquidationStore {
    pub orders: RwLock<HashMap<OrderId, VerifiedOrder>>,
}

pub struct Store {
    pub chains:            HashMap<ChainId, ChainStore>,
    pub liquidation_store: LiquidationStore,
    pub per_operator:      LocalWallet,
}
