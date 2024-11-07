#![allow(dead_code)]

use {
    super::{
        entities,
        repository,
    },
    crate::kernel::{
        db::DB,
        entities::{
            ChainId,
            ChainType,
            Evm,
            Svm,
        },
    },
    std::sync::Arc,
};

pub mod get_bids;

pub struct Config {
    pub chain_type: ChainType,
    pub chain_id:   ChainId,
}

pub trait ServiceTrait: entities::BidTrait + repository::BidTrait {}
impl ServiceTrait for Evm {
}
impl ServiceTrait for Svm {
}

pub struct Service<T: ServiceTrait> {
    config: Config,
    repo:   Arc<repository::Repository<T>>,
}

impl<T: ServiceTrait> Service<T> {
    pub fn new(db: DB, config: Config) -> Self {
        Self {
            repo: Arc::new(repository::Repository::new(db)),
            config,
        }
    }
}

#[derive(Clone)]
pub enum ServiceEnum {
    Evm(Arc<Service<Evm>>),
    Svm(Arc<Service<Svm>>),
}
