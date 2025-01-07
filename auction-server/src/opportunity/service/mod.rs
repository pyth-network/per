use {
    super::repository::{
        InMemoryStore,
        InMemoryStoreEvm,
        InMemoryStoreSvm,
        Repository,
    },
    crate::{
        auction::service::{
            self as auction_service,
        },
        kernel::{
            contracts::AdapterFactory,
            db::DB,
            entities::{
                ChainId,
                ChainType as ChainTypeEnum,
                Evm,
                Svm,
            },
            traced_client::TracedClient,
        },
        state::{
            ChainStoreEvm,
            ChainStoreSvm,
            Store,
        },
    },
    ethers::{
        providers::Provider,
        types::Address,
    },
    futures::future::try_join_all,
    std::{
        collections::HashMap,
        sync::Arc,
    },
    tokio::sync::RwLock,
};

pub mod add_opportunity;
pub mod get_config;
pub mod get_live_opportunities;
pub mod get_opportunities;
pub mod get_quote;
pub mod handle_opportunity_bid;
pub mod remove_invalid_or_expired_opportunities;
pub mod remove_opportunities;
pub mod verification;

mod get_spoof_info;
mod make_adapter_calldata;
mod make_opportunity_execution_params;
mod make_permitted_tokens;

// NOTE: Do not implement debug here. it has a circular reference to auction_service
pub struct ConfigEvm {
    pub adapter_factory_contract: Address,
    pub adapter_bytecode_hash:    [u8; 32],
    pub chain_id_num:             u64,
    pub permit2:                  Address,
    pub provider:                 Provider<TracedClient>,
    pub weth:                     Address,
    pub auction_service:          RwLock<Option<auction_service::Service<Evm>>>,
}

impl ConfigEvm {
    // TODO Move these to config trait?
    pub async fn inject_auction_service(&self, service: auction_service::Service<Evm>) {
        let mut write_gaurd = self.auction_service.write().await;
        *write_gaurd = Some(service);
    }
    pub async fn get_auction_service(&self) -> auction_service::Service<Evm> {
        self.auction_service
            .read()
            .await
            .clone()
            .expect("Failed to get auction service")
    }
}

// NOTE: Do not implement debug here. it has a circular reference to auction_service
pub struct ConfigSvm {
    pub auction_service: RwLock<Option<auction_service::Service<Svm>>>,
}

impl ConfigSvm {
    // TODO Move these to config trait?
    pub async fn inject_auction_service(&self, service: auction_service::Service<Svm>) {
        let mut write_gaurd = self.auction_service.write().await;
        *write_gaurd = Some(service);
    }
    pub async fn get_auction_service(&self) -> auction_service::Service<Svm> {
        self.auction_service
            .read()
            .await
            .clone()
            .expect("Failed to get auction service")
    }
}

#[allow(dead_code)]
pub trait Config: Send + Sync {}

impl Config for ConfigEvm {
}
impl Config for ConfigSvm {
}

impl ConfigEvm {
    async fn get_weth_address(
        adapter_contract: Address,
        provider: Provider<TracedClient>,
    ) -> anyhow::Result<Address> {
        let adapter = AdapterFactory::new(adapter_contract, Arc::new(provider));
        adapter
            .get_weth()
            .call()
            .await
            .map_err(|e| anyhow::anyhow!("Error getting WETH address from adapter: {:?}", e))
    }

    async fn get_adapter_bytecode_hash(
        adapter_contract: Address,
        provider: Provider<TracedClient>,
    ) -> anyhow::Result<[u8; 32]> {
        let adapter = AdapterFactory::new(adapter_contract, Arc::new(provider));
        adapter
            .get_opportunity_adapter_creation_code_hash()
            .call()
            .await
            .map_err(|e| anyhow::anyhow!("Error getting adapter code hash from adapter: {:?}", e))
    }

    async fn get_permit2_address(
        adapter_contract: Address,
        provider: Provider<TracedClient>,
    ) -> anyhow::Result<Address> {
        let adapter = AdapterFactory::new(adapter_contract, Arc::new(provider));
        adapter
            .get_permit_2()
            .call()
            .await
            .map_err(|e| anyhow::anyhow!("Error getting permit2 address from adapter: {:?}", e))
    }

    async fn try_new(
        adapter_factory_contract: Address,
        provider: Provider<TracedClient>,
        chain_id_num: u64,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            adapter_bytecode_hash: Self::get_adapter_bytecode_hash(
                adapter_factory_contract,
                provider.clone(),
            )
            .await?,
            permit2: Self::get_permit2_address(adapter_factory_contract, provider.clone()).await?,
            weth: Self::get_weth_address(adapter_factory_contract, provider.clone()).await?,
            adapter_factory_contract,
            chain_id_num,
            provider,
            auction_service: RwLock::new(None),
        })
    }

    pub async fn from_chains(
        chains: &HashMap<ChainId, ChainStoreEvm>,
    ) -> anyhow::Result<HashMap<ChainId, Self>> {
        let config_opportunity_service_evm = chains.iter().map(|(chain_id, chain_store)| {
            let chain_id_cloned = chain_id.clone();
            let adapter_factory_contract = chain_store.config.adapter_factory_contract;
            let provider_cloned = chain_store.provider.clone();
            async move {
                let config = Self::try_new(
                    adapter_factory_contract,
                    provider_cloned.clone(),
                    chain_store.network_id,
                )
                .await?;
                Ok::<(ChainId, Self), anyhow::Error>((chain_id_cloned, config))
            }
        });
        Ok(try_join_all(config_opportunity_service_evm)
            .await?
            .into_iter()
            .collect())
    }
}

impl ConfigSvm {
    pub async fn from_chains(
        chains: &HashMap<ChainId, ChainStoreSvm>,
    ) -> anyhow::Result<HashMap<ChainId, Self>> {
        Ok(chains
            .iter()
            .map(|(chain_id, _)| {
                (
                    chain_id.clone(),
                    Self {
                        auction_service: RwLock::new(None),
                    },
                )
            })
            .collect())
    }
}

pub trait ChainType: Send + Sync {
    type Config: Config;
    type InMemoryStore: InMemoryStore;

    fn get_type() -> ChainTypeEnum;
}

pub struct ChainTypeEvm;
pub struct ChainTypeSvm;

impl ChainType for ChainTypeEvm {
    type Config = ConfigEvm;
    type InMemoryStore = InMemoryStoreEvm;

    fn get_type() -> ChainTypeEnum {
        ChainTypeEnum::Evm
    }
}

impl ChainType for ChainTypeSvm {
    type Config = ConfigSvm;
    type InMemoryStore = InMemoryStoreSvm;

    fn get_type() -> ChainTypeEnum {
        ChainTypeEnum::Svm
    }
}

// TODO maybe just create a service per chain_id?
pub struct Service<T: ChainType> {
    store:  Arc<Store>,
    db:     DB,
    // TODO maybe after adding state for opportunity we can remove the arc
    repo:   Arc<Repository<T::InMemoryStore>>,
    config: HashMap<ChainId, T::Config>,
}

impl<T: ChainType> Service<T> {
    pub fn new(store: Arc<Store>, db: DB, config: HashMap<ChainId, T::Config>) -> Self {
        Self {
            store,
            db,
            repo: Arc::new(Repository::<T::InMemoryStore>::new()),
            config,
        }
    }
}
