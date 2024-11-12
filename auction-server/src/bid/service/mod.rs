#![allow(dead_code)]

use {
    super::{
        entities,
        repository,
    },
    crate::{
        kernel::{
            db::DB,
            entities::{
                ChainId,
                ChainType,
                Evm,
                Svm,
            },
        },
        state::StoreNew,
    },
    solana_client::nonblocking::rpc_client::RpcClient,
    solana_sdk::{
        pubkey::Pubkey,
        signature::Keypair,
    },
    std::sync::{
        Arc,
        Weak,
    },
};

pub mod get_bid;
pub mod get_bids;
pub mod get_live_bids;
pub mod handle_bid;
mod verification;

pub struct ConfigSvm {
    pub express_relay_program_id:      Pubkey,
    pub client:                        RpcClient,
    pub wallet_program_router_account: Pubkey,
    pub relayer:                       Keypair,
}

pub struct ConfigEvm {}

pub struct Config<T> {
    pub chain_type: ChainType,
    pub chain_id:   ChainId,

    pub chain_config: T,
}

pub trait ServiceTrait:
    entities::BidTrait + entities::BidCreateTrait + repository::RepositoryTrait
{
    type ConfigType;
}
impl ServiceTrait for Evm {
    type ConfigType = ConfigEvm;
}
impl ServiceTrait for Svm {
    type ConfigType = ConfigSvm;
}

pub struct Service<T: ServiceTrait> {
    store:  Weak<StoreNew>,
    config: Config<T::ConfigType>,
    repo:   Arc<repository::Repository<T>>,
}

impl<T: ServiceTrait> Service<T> {
    pub fn new(db: DB, config: Config<T::ConfigType>) -> Self {
        Self {
            repo: Arc::new(repository::Repository::new(db, config.chain_id.clone())),
            config,
            store: Weak::new(),
        }
    }

    pub fn set_store(&mut self, store: Arc<StoreNew>) {
        self.store = Arc::downgrade(&store);
    }

    pub fn get_store(&self) -> Arc<StoreNew> {
        self.store.upgrade().expect("Store is missing")
    }
}

#[derive(Clone)]
pub enum ServiceEnum {
    Evm(Arc<Service<Evm>>),
    Svm(Arc<Service<Svm>>),
}
