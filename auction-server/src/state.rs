use std::collections::HashMap;
use ethers::{signers::LocalWallet, types::{Bytes, Address, U256}, providers::{Provider, Http}};
use tokio::sync::RwLock;

use crate::config::{ChainId, EthereumConfig};

pub type PermissionKey = Bytes;


#[derive(Clone)]
pub struct SimulatedBid {
    pub contract: Address,
    pub calldata: Bytes,
    pub bid: U256,
    // simulation_time: 
}

pub struct ChainStore {
    pub provider: Provider<Http>,
    pub network_id: u64,
    pub config: EthereumConfig,
    pub bids: RwLock<HashMap<PermissionKey, Vec<SimulatedBid>>>,
}


pub struct Store {
    pub chains: HashMap<ChainId, ChainStore>,
    pub per_operator: LocalWallet,
}