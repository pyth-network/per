use {
    super::{
        contracts::AdapterFactory,
        entities,
        repository::{
            models::{
                OpportunityMetadata,
                OpportunityMetadataEvm,
                OpportunityMetadataSvm,
            },
            InMemoryStore,
            InMemoryStoreEvm,
            InMemoryStoreSvm,
            Repository,
        },
    },
    crate::{
        api::RestError,
        kernel::{
            db::DB,
            entities::ChainId,
        },
        state::{
            ChainStoreEvm,
            ChainStoreSvm,
            Store,
        },
        traced_client::TracedClient,
    },
    axum::async_trait,
    ethers::{
        abi::Item,
        providers::Provider,
        types::Address,
    },
    futures::future::try_join_all,
    std::{
        collections::HashMap,
        sync::Arc,
    },
};

pub mod add_opportunity;
pub mod get_config;
pub mod get_opportunities;
pub mod handle_opportunity_bid;
pub mod remove_invalid_or_expired_opportunities;

mod get_spoof_info;
mod make_adapter_calldata;
mod make_opportunity_execution_params;
mod make_permitted_tokens;
mod verification;

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
            .map(|(chain_id, _)| (chain_id.clone(), Self {}))
            .collect())
    }
}

pub trait ChainType {
    type Config: Config;
    type InMemoryStore: InMemoryStore;
}

pub struct ChainTypeEvm;
pub struct ChainTypeSvm;

impl ChainType for ChainTypeEvm {
    type Config = ConfigEvm;
    type InMemoryStore = InMemoryStoreEvm;
}

impl ChainType for ChainTypeSvm {
    type Config = ConfigSvm;
    type InMemoryStore = InMemoryStoreSvm;
}

pub struct Service<T: ChainType> {
    store:  Arc<Store>,
    db:     DB,
    repo:   Repository<T::InMemoryStore>,
    config: HashMap<ChainId, T::Config>,
}

impl<T: ChainType> Service<T> {
    pub fn new(store: Arc<Store>, db: DB, config: HashMap<ChainId, T::Config>) -> Self {
        Self {
            store,
            db,
            repo: Repository::<T::InMemoryStore>::new(),
            config,
        }
    }
}
