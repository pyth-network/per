use {
    crate::{
        api::{
            ws::WsState,
            RestError,
        },
        auction::service as auction_service,
        config::{
            ChainId,
            ConfigEvm,
            ConfigSvm,
        },
        kernel::traced_client::TracedClient,
        models,
        opportunity::service as opportunity_service,
    },
    anyhow::anyhow,
    axum_prometheus::metrics_exporter_prometheus::PrometheusHandle,
    base64::{
        engine::general_purpose::URL_SAFE_NO_PAD,
        Engine,
    },
    ethers::{
        middleware::Middleware,
        prelude::BlockNumber,
        providers::Provider,
        types::U256,
    },
    rand::Rng,
    solana_client::{
        nonblocking::rpc_client::RpcClient,
        rpc_response::{
            Response,
            RpcLogsResponse,
        },
    },
    solana_sdk::pubkey::Pubkey,
    std::{
        collections::HashMap,
        sync::Arc,
        time::Duration,
    },
    tokio::sync::{
        broadcast::{
            self,
            Receiver,
            Sender,
        },
        RwLock,
    },
    uuid::Uuid,
};

pub type GetOrCreate<T> = (T, bool);

pub struct ChainStoreEvm {
    pub provider:        Provider<TracedClient>,
    pub network_id:      u64,
    // TODO move this to core fields
    pub config:          ConfigEvm,
    pub block_gas_limit: U256,
}

impl ChainStoreEvm {
    pub fn get_chain_provider(
        chain_id: &String,
        chain_config: &ConfigEvm,
    ) -> anyhow::Result<Provider<TracedClient>> {
        let mut provider = TracedClient::new(
            chain_id.clone(),
            &chain_config.geth_rpc_addr,
            chain_config.rpc_timeout,
        )
        .map_err(|err| {
            tracing::error!(
                "Failed to create provider for chain({chain_id}) at {rpc_addr}: {:?}",
                err,
                chain_id = chain_id,
                rpc_addr = chain_config.geth_rpc_addr
            );
            anyhow!(
                "Failed to connect to chain({chain_id}) at {rpc_addr}: {:?}",
                err,
                chain_id = chain_id,
                rpc_addr = chain_config.geth_rpc_addr
            )
        })?;
        provider.set_interval(Duration::from_secs(chain_config.poll_interval));
        Ok(provider)
    }
    pub async fn create_store(chain_id: String, config: ConfigEvm) -> anyhow::Result<Self> {
        let provider = Self::get_chain_provider(&chain_id, &config)?;

        let id = provider.get_chainid().await?.as_u64();
        let block = provider
            .get_block(BlockNumber::Latest)
            .await?
            .expect("Failed to get latest block");

        Ok(Self {
            provider,
            network_id: id,
            config: config.clone(),
            block_gas_limit: block.gas_limit,
        })
    }
}

pub struct ChainStoreSvm {
    pub log_sender:          Sender<Response<RpcLogsResponse>>,
    // only to avoid closing the channel
    pub _dummy_log_receiver: Receiver<Response<RpcLogsResponse>>,
    pub config:              ConfigSvm,
    pub token_program_cache: RwLock<HashMap<Pubkey, Pubkey>>,
}

impl ChainStoreSvm {
    pub fn new(config: ConfigSvm) -> Self {
        let (tx, rx) = broadcast::channel(1000);

        Self {
            log_sender: tx,
            _dummy_log_receiver: rx,
            config,
            token_program_cache: RwLock::new(HashMap::new()),
        }
    }

    pub async fn get_token_program(
        &self,
        mint: Pubkey,
        rpc_client: &RpcClient,
    ) -> anyhow::Result<Pubkey> {
        if let Some(program) = self.token_program_cache.read().await.get(&mint) {
            return Ok(*program);
        }

        let program = rpc_client
            .get_account(&mint)
            .await
            .map_err(|err| {
                tracing::error!(
                    "Failed to retrieve owner program for mint account {mint}: {:?}",
                    err,
                    mint = mint
                );
                anyhow!(
                    "Failed to retrieve owner program for mint account {mint}: {:?}",
                    err,
                    mint = mint
                )
            })?
            .owner;

        self.token_program_cache.write().await.insert(mint, program);
        Ok(program)
    }
}

pub struct Store {
    pub chains_evm:       HashMap<ChainId, Arc<ChainStoreEvm>>,
    pub chains_svm:       HashMap<ChainId, Arc<ChainStoreSvm>>,
    pub ws:               WsState,
    pub db:               sqlx::PgPool,
    pub secret_key:       String,
    pub access_tokens:    RwLock<HashMap<models::AccessTokenToken, models::Profile>>,
    pub metrics_recorder: PrometheusHandle,
}

pub struct StoreNew {
    pub opportunity_service_evm:
        Arc<opportunity_service::Service<opportunity_service::ChainTypeEvm>>,
    pub opportunity_service_svm:
        Arc<opportunity_service::Service<opportunity_service::ChainTypeSvm>>,
    pub store:                   Arc<Store>,

    auction_services: HashMap<ChainId, auction_service::ServiceEnum>,
}

impl StoreNew {
    pub fn new(
        store: Arc<Store>,
        opportunity_service_evm: Arc<
            opportunity_service::Service<opportunity_service::ChainTypeEvm>,
        >,
        opportunity_service_svm: Arc<
            opportunity_service::Service<opportunity_service::ChainTypeSvm>,
        >,
        auction_services: HashMap<ChainId, auction_service::ServiceEnum>,
    ) -> Self {
        Self {
            opportunity_service_evm,
            opportunity_service_svm,
            store,
            auction_services,
        }
    }

    pub fn get_auction_service(
        &self,
        chain_id: &ChainId,
    ) -> Result<auction_service::ServiceEnum, RestError> {
        self.auction_services
            .get(chain_id)
            .cloned()
            .ok_or(RestError::InvalidChainId)
    }

    // TODO remove this after deprecating the old bid apis
    pub fn get_all_auction_services(&self) -> &HashMap<ChainId, auction_service::ServiceEnum> {
        &self.auction_services
    }
}

impl Store {
    pub async fn create_profile(
        &self,
        create_profile: express_relay_api_types::profile::CreateProfile,
    ) -> Result<models::Profile, RestError> {
        let id = Uuid::new_v4();
        let role: models::ProfileRole = create_profile.role.clone().into();
        let profile: models::Profile = sqlx::query_as(
            "INSERT INTO profile (id, name, email, role) VALUES ($1, $2, $3, $4) RETURNING id, name, email, role, created_at, updated_at",
        ).bind(id)
        .bind(create_profile.name.clone())
        .bind(create_profile.email.to_string())
        .bind(role)
        .fetch_one(&self.db).await
        .map_err(|e| {
            if let Some(true) = e.as_database_error().map(|e| e.is_unique_violation()) {
                return RestError::BadParameters("Profile with this email already exists".to_string());
            }
            tracing::error!("DB: Failed to insert profile: {} - profile_data: {:?}", e, create_profile);
            RestError::TemporarilyUnavailable
        })?;
        Ok(profile)
    }

    fn generate_url_safe_token(&self) -> anyhow::Result<String> {
        let mut rng = rand::thread_rng();
        let bytes: [u8; 32] = rng.gen();
        Ok(URL_SAFE_NO_PAD.encode(bytes))
    }

    pub async fn get_profile_by_email(
        &self,
        email: models::EmailAddress,
    ) -> Result<Option<models::Profile>, RestError> {
        sqlx::query_as("SELECT * FROM profile WHERE email = $1")
            .bind(email.0.to_string())
            .fetch_optional(&self.db)
            .await
            .map_err(|e| {
                tracing::error!("DB: Failed to fetch profile: {} - email: {}", e, email.0);
                RestError::TemporarilyUnavailable
            })
    }

    pub async fn get_profile_by_id(
        &self,
        id: models::ProfileId,
    ) -> Result<Option<models::Profile>, RestError> {
        sqlx::query_as("SELECT * FROM profile WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.db)
            .await
            .map_err(|e| {
                tracing::error!("DB: Failed to fetch profile: {} - id: {}", e, id);
                RestError::TemporarilyUnavailable
            })
    }

    pub async fn get_or_create_access_token(
        &self,
        profile_id: models::ProfileId,
    ) -> Result<GetOrCreate<models::AccessToken>, RestError> {
        let generated_token = self.generate_url_safe_token().map_err(|e| {
            tracing::error!(
                "Failed to generate access token: {} - profile_id: {}",
                e,
                profile_id
            );
            RestError::TemporarilyUnavailable
        })?;

        let id = Uuid::new_v4();
        let result = sqlx::query!(
            "INSERT INTO access_token (id, profile_id, token)
        SELECT $1, $2, $3
        WHERE NOT EXISTS (
            SELECT id
            FROM access_token
            WHERE profile_id = $2 AND revoked_at is NULL
        );",
            id,
            profile_id,
            generated_token
        )
        .execute(&self.db)
        .await
        .map_err(|e| {
            tracing::error!(
                "DB: Failed to create access token: {} - profile_id: {}",
                e,
                profile_id
            );
            RestError::TemporarilyUnavailable
        })?;

        let token = sqlx::query_as!(
            models::AccessToken,
            "SELECT * FROM access_token
        WHERE profile_id = $1 AND revoked_at is NULL;",
            profile_id,
        )
        .fetch_one(&self.db)
        .await
        .map_err(|e| {
            tracing::error!(
                "DB: Failed to fetch access token: {} - profile_id: {}",
                e,
                profile_id
            );
            RestError::TemporarilyUnavailable
        })?;

        let profile = self
            .get_profile_by_id(profile_id)
            .await?
            .ok_or_else(|| RestError::BadParameters("Profile id not found".to_string()))?;
        self.access_tokens
            .write()
            .await
            .insert(token.token.clone(), profile);
        Ok((token, result.rows_affected() > 0))
    }

    pub async fn revoke_access_token(
        &self,
        token: &models::AccessTokenToken,
    ) -> Result<(), RestError> {
        sqlx::query!(
            "UPDATE access_token
        SET revoked_at = now()
        WHERE token = $1 AND revoked_at is NULL;",
            token
        )
        .execute(&self.db)
        .await
        .map_err(|e| {
            tracing::error!("DB: Failed to revoke access token: {}", e);
            RestError::TemporarilyUnavailable
        })?;

        self.access_tokens.write().await.remove(token);
        Ok(())
    }

    pub async fn get_profile_by_token(
        &self,
        token: &models::AccessTokenToken,
    ) -> Result<models::Profile, RestError> {
        self.access_tokens
            .read()
            .await
            .get(token)
            .cloned()
            .ok_or(RestError::InvalidToken)
    }
}
