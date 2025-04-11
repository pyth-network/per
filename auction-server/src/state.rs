#[double]
use crate::opportunity::service::Service as OpportunityService;
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
        models,
        opportunity::service as opportunity_service,
    },
    axum_prometheus::metrics_exporter_prometheus::PrometheusHandle,
    base64::{
        engine::general_purpose::URL_SAFE_NO_PAD,
        Engine,
    },
    mockall_double::double,
    rand::Rng,
    solana_client::rpc_response::{
        Response,
        RpcLogsResponse,
    },
    std::{
        collections::HashMap,
        sync::Arc,
    },
    tokio::sync::{
        broadcast::{
            self,
            Receiver,
            Sender,
        },
        RwLock,
    },
    tokio_util::task::TaskTracker,
    tracing::{
        info_span,
        Instrument,
    },
    uuid::Uuid,
};

pub type GetOrCreate<T> = (T, bool);

pub struct ChainStoreEvm {}

impl ChainStoreEvm {
    pub async fn create_store(_chain_id: String, _config: ConfigEvm) -> anyhow::Result<Self> {
        Ok(Self {})
    }
}

pub struct ChainStoreSvm {
    pub log_sender:          Sender<Response<RpcLogsResponse>>,
    // only to avoid closing the channel
    pub _dummy_log_receiver: Receiver<Response<RpcLogsResponse>>,
    pub config:              ConfigSvm,
}

impl ChainStoreSvm {
    pub fn new(config: ConfigSvm) -> Self {
        let (tx, rx) = broadcast::channel(1000);

        Self {
            log_sender: tx,
            _dummy_log_receiver: rx,
            config,
        }
    }
}

pub struct ServerState {
    pub metrics_recorder: PrometheusHandle,
}

pub struct Store {
    pub chains_evm:    HashMap<ChainId, Arc<ChainStoreEvm>>,
    pub chains_svm:    HashMap<ChainId, Arc<ChainStoreSvm>>,
    pub ws:            WsState,
    pub db:            sqlx::PgPool,
    pub secret_key:    String,
    pub access_tokens: RwLock<HashMap<models::AccessTokenToken, models::Profile>>,
}

pub struct StoreNew {
    pub opportunity_service_svm: Arc<OpportunityService<opportunity_service::ChainTypeSvm>>,
    pub store:                   Arc<Store>,
    pub task_tracker:            TaskTracker,

    auction_services: HashMap<ChainId, auction_service::ServiceEnum>,
}

impl StoreNew {
    pub fn new(
        store: Arc<Store>,
        opportunity_service_svm: Arc<OpportunityService<opportunity_service::ChainTypeSvm>>,
        auction_services: HashMap<ChainId, auction_service::ServiceEnum>,
        task_tracker: TaskTracker,
    ) -> Self {
        Self {
            opportunity_service_svm,
            store,
            auction_services,
            task_tracker,
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
        .fetch_one(&self.db)
            .instrument(info_span!("db_create_profile")).
            await
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
            .instrument(info_span!("db_get_profile_by_email"))
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
            .instrument(info_span!("db_get_profile_by_id"))
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
        .instrument(info_span!("db_get_or_create_access_token"))
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
        .instrument(info_span!("db_get_or_create_access_token"))
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
        .instrument(info_span!("db_revoke_access_token"))
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
