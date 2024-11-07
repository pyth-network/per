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
        },
    },
    std::sync::Arc,
};

pub struct Config {
    pub chain_type: ChainType,
    pub chain_id:   ChainId,
}

pub struct Service<T: entities::BidTrait> {
    config: Config,
    repo:   Arc<repository::Repository<T>>,
}

impl<T: entities::BidTrait> Service<T> {
    pub fn new(db: DB, config: Config) -> Self {
        Self {
            repo: Arc::new(repository::Repository::new(db)),
            config,
        }
    }
}
