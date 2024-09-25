use {
    super::repository::{
        Cache,
        CacheEvm,
        CacheSvm,
        Repository,
    },
    crate::{
        kernel::{
            db::DB,
            entities::ChainId,
        },
        state::Store,
        traced_client::TracedClient,
    },
    ethers::{
        providers::Provider,
        types::Address,
    },
    std::{
        collections::HashMap,
        sync::Arc,
    },
};

pub mod add_opportunity;
pub mod get_config;
pub mod get_opportunities;
pub mod get_spoof_info;
pub mod handle_opportunity_bid;
pub mod make_adapter_calldata;
pub mod make_opportunity_execution_params;
pub mod make_permitted_tokens;
pub mod remove_invalid_or_expired_opportunities;
pub mod verify_opportunity;

#[derive(Debug)]
pub struct ConfigEvm {
    pub adapter_factory_contract: Address,
    pub adapter_bytecode_hash:    [u8; 32],
    pub chain_id_num:             u64,
    pub permit2:                  Address,
    pub provider:                 Provider<TracedClient>,
    pub weth:                     Address,
}

#[derive(Debug)]
pub struct ConfigSvm {}

pub trait Config {}

impl Config for ConfigEvm {
}
impl Config for ConfigSvm {
}

pub trait ChainType {
    type Config: Config;
    type Cache: Cache;
}

pub struct ChainTypeEvm;
pub struct ChainTypeSvm;

impl ChainType for ChainTypeEvm {
    type Config = ConfigEvm;
    type Cache = CacheEvm;
}

impl ChainType for ChainTypeSvm {
    type Config = ConfigSvm;
    type Cache = CacheSvm;
}

pub struct Service<T: ChainType> {
    store:  Arc<Store>,
    db:     DB,
    repo:   Repository<T::Cache>,
    config: HashMap<ChainId, T::Config>,
}

impl<T: ChainType> Service<T> {
    pub fn new(store: Arc<Store>, db: DB, config: HashMap<ChainId, T::Config>) -> Arc<Self> {
        Arc::new(Service {
            store,
            db,
            repo: Repository::<T::Cache>::new(),
            config,
        })
    }
}
