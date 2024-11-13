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
            traced_client::TracedClient,
        },
        opportunity::service as opportunity_service,
    },
    ethers::{
        core::k256::ecdsa::SigningKey,
        providers::Provider,
        signers::Wallet,
        types::{
            Address,
            U256,
        },
    },
    gas_oracle::EthProviderOracle,
    solana_client::nonblocking::rpc_client::RpcClient,
    solana_sdk::{
        pubkey::Pubkey,
        signature::Keypair,
    },
    std::sync::Arc,
};

pub mod get_bid;
pub mod get_bids;
pub mod get_live_bids;
pub mod handle_bid;
mod verification;

pub struct ExpressRelaySvm {
    pub program_id:                  Pubkey,
    pub relayer:                     Keypair,
    pub permission_account_position: usize,
    pub router_account_position:     usize,
}

pub struct ConfigSvm {
    pub client:                        RpcClient,
    pub wallet_program_router_account: Pubkey,
    pub express_relay:                 ExpressRelaySvm,
}

pub struct ExpressRelayEvm {
    pub contract_address: Address,
    pub relayer:          Wallet<SigningKey>,
}

type GasOracleType = EthProviderOracle<Provider<TracedClient>>;
pub struct ConfigEvm {
    pub express_relay:   ExpressRelayEvm,
    pub provider:        Provider<TracedClient>,
    pub block_gas_limit: U256,
    pub oracle:          GasOracleType,
}

impl ConfigEvm {
    pub fn new(
        express_relay: ExpressRelayEvm,
        provider: Provider<TracedClient>,
        block_gas_limit: U256,
    ) -> Self {
        Self {
            express_relay,
            block_gas_limit,
            oracle: GasOracleType::new(provider.clone()),
            provider,
        }
    }
}

pub struct Config<T> {
    pub chain_type: ChainType,
    pub chain_id:   ChainId,

    pub chain_config: T,
}

pub trait ServiceTrait:
    entities::BidTrait + entities::BidCreateTrait + repository::RepositoryTrait
{
    type ConfigType;
    type OpportunityServiceType: opportunity_service::ChainType;
}
impl ServiceTrait for Evm {
    type ConfigType = ConfigEvm;
    type OpportunityServiceType = opportunity_service::ChainTypeEvm;
}
impl ServiceTrait for Svm {
    type ConfigType = ConfigSvm;
    type OpportunityServiceType = opportunity_service::ChainTypeSvm;
}

pub struct Service<T: ServiceTrait> {
    opportunity_service: Arc<opportunity_service::Service<T::OpportunityServiceType>>,
    config:              Config<T::ConfigType>,
    repo:                Arc<repository::Repository<T>>,
}

impl<T: ServiceTrait> Service<T> {
    pub fn new(
        db: DB,
        config: Config<T::ConfigType>,
        opportunity_service: Arc<opportunity_service::Service<T::OpportunityServiceType>>,
    ) -> Self {
        Self {
            repo: Arc::new(repository::Repository::new(db, config.chain_id.clone())),
            config,
            opportunity_service,
        }
    }
}

#[derive(Clone)]
pub enum ServiceEnum {
    Evm(Arc<Service<Evm>>),
    Svm(Arc<Service<Svm>>),
}
