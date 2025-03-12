#[cfg(test)]
use mockall::automock;
use {
    super::{
        entities,
        models,
        InMemoryStore,
    },
    crate::{
        api::RestError,
        kernel::{
            db::DB,
            entities::{
                ChainId,
                PermissionKey,
            },
        },
        models::ChainType,
        opportunity::entities::{
            FeeToken,
            Opportunity as OpportunityTrait,
            TokenAccountInitializationConfigs,
        },
    },
    axum::async_trait,
    ethers::types::{
        Address,
        Bytes,
        U256,
    },
    serde::{
        de::DeserializeOwned,
        Deserialize,
        Serialize,
    },
    serde_with::{
        base64::Base64,
        serde_as,
        DisplayFromStr,
    },
    solana_sdk::{
        clock::Slot,
        pubkey::Pubkey,
    },
    sqlx::{
        prelude::FromRow,
        types::{
            Json,
            JsonValue,
        },
        QueryBuilder,
    },
    std::fmt::Debug,
    time::{
        OffsetDateTime,
        PrimitiveDateTime,
    },
    tracing::{
        info_span,
        Instrument,
    },
    uuid::Uuid,
};

#[derive(Clone, Debug, PartialEq, PartialOrd, sqlx::Type)]
#[sqlx(type_name = "opportunity_removal_reason", rename_all = "lowercase")]
pub enum OpportunityRemovalReason {
    Expired,
    Invalid,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OpportunityMetadataEvm {
    pub target_contract:   Address,
    #[serde(with = "express_relay_api_types::serde::u256")]
    pub target_call_value: U256,
    pub target_calldata:   Bytes,
}

#[serde_as]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OpportunityMetadataSvmProgramLimo {
    #[serde_as(as = "Base64")]
    pub order:         Vec<u8>,
    #[serde_as(as = "DisplayFromStr")]
    pub order_address: Pubkey,
    pub slot:          Slot,
}

#[serde_as]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OpportunityMetadataSvmProgramSwap {
    #[serde_as(as = "DisplayFromStr")]
    pub user_wallet_address:                 Pubkey,
    pub fee_token:                           FeeToken,
    pub referral_fee_bps:                    u16,
    pub platform_fee_bps:                    u64,
    #[serde_as(as = "DisplayFromStr")]
    pub token_program_user:                  Pubkey,
    #[serde_as(as = "DisplayFromStr")]
    pub token_program_searcher:              Pubkey,
    pub user_mint_user_balance:              u64,
    pub token_account_initialization_config: TokenAccountInitializationConfigs,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "program", rename_all = "lowercase")]
pub enum OpportunityMetadataSvmProgram {
    Limo(OpportunityMetadataSvmProgramLimo),
    Swap(OpportunityMetadataSvmProgramSwap),
}

#[serde_as]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OpportunityMetadataSvm {
    #[serde(flatten)]
    pub program:            OpportunityMetadataSvmProgram,
    #[serde_as(as = "DisplayFromStr")]
    pub router:             Pubkey,
    #[serde_as(as = "DisplayFromStr")]
    pub permission_account: Pubkey,
}

pub trait OpportunityMetadata:
    Debug + Clone + Serialize + DeserializeOwned + Send + Sync + Unpin + 'static
{
    fn get_chain_type() -> ChainType;
}

impl OpportunityMetadata for OpportunityMetadataEvm {
    fn get_chain_type() -> ChainType {
        ChainType::Evm
    }
}

impl OpportunityMetadata for OpportunityMetadataSvm {
    fn get_chain_type() -> ChainType {
        ChainType::Svm
    }
}

// TODO Update metdata to exection_params
#[derive(Clone, FromRow, Debug)]
pub struct Opportunity<T: OpportunityMetadata> {
    pub id:             Uuid,
    pub creation_time:  PrimitiveDateTime,
    pub permission_key: Vec<u8>,
    pub chain_id:       String,
    pub chain_type:     ChainType,
    pub removal_time:   Option<PrimitiveDateTime>,
    pub sell_tokens:    JsonValue,
    pub buy_tokens:     JsonValue,
    pub removal_reason: Option<OpportunityRemovalReason>,
    pub metadata:       Json<T>,
}

#[cfg_attr(test, automock)]
#[async_trait]
pub trait Database<T: InMemoryStore>: Debug + Send + Sync + 'static {
    async fn add_opportunity(&self, opportunity: &T::Opportunity) -> Result<(), RestError>;
    async fn get_opportunities(
        &self,
        chain_id: ChainId,
        permission_key: Option<PermissionKey>,
        from_time: Option<OffsetDateTime>,
    ) -> Result<Vec<T::Opportunity>, RestError>;
    async fn remove_opportunities(
        &self,
        permission_key: PermissionKey,
        chain_id: ChainId,
        reason: OpportunityRemovalReason,
    ) -> anyhow::Result<()>;
    async fn remove_opportunity(
        &self,
        opportunity: &T::Opportunity,
        reason: OpportunityRemovalReason,
    ) -> anyhow::Result<()>;
}
#[async_trait]
impl<T: InMemoryStore> Database<T> for DB {
    async fn add_opportunity(&self, opportunity: &T::Opportunity) -> Result<(), RestError> {
        let metadata = opportunity.get_models_metadata();
        let chain_type = <T::Opportunity as entities::Opportunity>::ModelMetadata::get_chain_type();
        sqlx::query!("INSERT INTO opportunity (id,
                                                        creation_time,
                                                        permission_key,
                                                        chain_id,
                                                        chain_type,
                                                        metadata,
                                                        sell_tokens,
                                                        buy_tokens) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        opportunity.id,
        PrimitiveDateTime::new(opportunity.creation_time.date(), opportunity.creation_time.time()),
        opportunity.permission_key.to_vec(),
        opportunity.chain_id,
        chain_type as _,
        serde_json::to_value(metadata).expect("Failed to serialize metadata"),
        serde_json::to_value(&opportunity.sell_tokens).expect("Failed to serialize sell_tokens"),
        serde_json::to_value(&opportunity.buy_tokens).expect("Failed to serialize buy_tokens"))
            .execute(self)
            .instrument(info_span!("db_add_opportunity"))
            .await
            .map_err(|e| {
                tracing::error!("DB: Failed to insert opportunity: {}", e);
                RestError::TemporarilyUnavailable
            })?;
        Ok(())
    }

    async fn get_opportunities(
        &self,
        chain_id: ChainId,
        permission_key: Option<PermissionKey>,
        from_time: Option<OffsetDateTime>,
    ) -> Result<Vec<<T as InMemoryStore>::Opportunity>, RestError> {
        let mut query = QueryBuilder::new("SELECT * from opportunity WHERE chain_type = ");
        query.push_bind(
            <<T::Opportunity as entities::Opportunity>::ModelMetadata>::get_chain_type(),
        );
        query.push(" AND chain_id = ");
        query.push_bind(chain_id.clone());
        if let Some(permission_key) = permission_key.clone() {
            query.push(" AND permission_key = ");
            query.push_bind(permission_key.to_vec());
        }
        if let Some(from_time) = from_time {
            query.push(" AND creation_time >= ");
            query.push_bind(from_time);
        }
        query.push(" ORDER BY creation_time ASC LIMIT ");
        query.push_bind(super::OPPORTUNITY_PAGE_SIZE_CAP as i64);
        let opps: Vec<models::Opportunity<<T::Opportunity as entities::Opportunity>::ModelMetadata>> = query
            .build_query_as()
            .fetch_all(self)
            .instrument(info_span!("db_get_opportunities"))
            .await
            .map_err(|e| {
                tracing::error!(
                    "DB: Failed to fetch opportunities: {} - chain_id: {:?} - permission_key: {:?} - from_time: {:?}",
                    e,
                    chain_id,
                    permission_key,
                    from_time,
                );
                RestError::TemporarilyUnavailable
            })?;

        opps.into_iter().map(|opp| opp.clone().try_into().map_err(
            |_| {
                tracing::error!(
                    "Failed to convert database opportunity to entity opportunity: {:?} - chain_id: {:?} - permission_key: {:?} - from_time: {:?}",
                    opp,
                    chain_id,
                    permission_key,
                    from_time,
                );
                RestError::TemporarilyUnavailable
            }
        )).collect()
    }

    async fn remove_opportunities(
        &self,
        permission_key: PermissionKey,
        chain_id: ChainId,
        reason: OpportunityRemovalReason,
    ) -> anyhow::Result<()> {
        let now = OffsetDateTime::now_utc();
        sqlx::query("UPDATE opportunity SET removal_time = $1, removal_reason = $2 WHERE permission_key = $3 AND chain_id = $4 and removal_time IS NULL")
            .bind(PrimitiveDateTime::new(now.date(), now.time()))
            .bind(reason)
            .bind(permission_key.as_ref())
            .bind(chain_id)
            .execute(self)
            .instrument(info_span!("db_remove_opportunities"))
            .await?;
        Ok(())
    }

    async fn remove_opportunity(
        &self,
        opportunity: &T::Opportunity,
        reason: OpportunityRemovalReason,
    ) -> anyhow::Result<()> {
        let now = OffsetDateTime::now_utc();
        sqlx::query("UPDATE opportunity SET removal_time = $1, removal_reason = $2 WHERE id = $3 AND removal_time IS NULL")
            .bind(PrimitiveDateTime::new(now.date(), now.time()))
            .bind(reason)
            .bind(opportunity.id)
            .execute(self)
            .instrument(info_span!("db_remove_opportunity"))
            .await?;
        Ok(())
    }
}
