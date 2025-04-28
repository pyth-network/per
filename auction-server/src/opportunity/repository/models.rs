#[cfg(test)]
use mockall::automock;
use {
    super::models,
    crate::{
        api::RestError,
        kernel::{
            db::DB,
            entities::{
                ChainId,
                PermissionKeySvm,
            },
        },
        models::ChainType,
        opportunity::entities::{
            FeeToken,
            OpportunitySvm,
            TokenAccountInitializationConfigs,
        },
    },
    axum::async_trait,
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
    tracing::instrument,
    uuid::Uuid,
};

#[derive(Clone, Debug, PartialEq, PartialOrd, sqlx::Type)]
#[sqlx(type_name = "opportunity_removal_reason", rename_all = "lowercase")]
pub enum OpportunityRemovalReason {
    Expired,
    Invalid,
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
    pub user_wallet_address:                  Pubkey,
    pub fee_token:                            FeeToken,
    #[serde(default = "default_referral_fee_ppm")]
    pub referral_fee_ppm:                     u64,
    pub referral_fee_bps:                     u16,
    pub platform_fee_bps:                     u64,
    #[serde_as(as = "DisplayFromStr")]
    #[serde(default = "default_token_program")]
    pub token_program_user:                   Pubkey,
    #[serde_as(as = "DisplayFromStr")]
    #[serde(default = "default_token_program")]
    pub token_program_searcher:               Pubkey,
    #[serde(default)]
    pub user_mint_user_balance:               u64,
    #[serde(default = "TokenAccountInitializationConfigs::searcher_payer")]
    pub token_account_initialization_configs: TokenAccountInitializationConfigs,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memo:                                 Option<String>,
    #[serde(default = "default_cancellable")]
    pub cancellable:                          bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimum_lifetime:                     Option<u32>,
}

fn default_cancellable() -> bool {
    true
}

fn default_token_program() -> Pubkey {
    spl_token::ID
}

fn default_referral_fee_ppm() -> u64 {
    0
}


#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "program", rename_all = "lowercase")]
pub enum OpportunityMetadataSvmProgram {
    Limo(OpportunityMetadataSvmProgramLimo),
    Swap(OpportunityMetadataSvmProgramSwap),
}

#[serde_as]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
pub trait Database: Debug + Send + Sync + 'static {
    async fn add_opportunity(&self, opportunity: &OpportunitySvm) -> Result<(), RestError>;
    async fn get_opportunities(
        &self,
        chain_id: ChainId,
        from_time: Option<OffsetDateTime>,
    ) -> Result<Vec<OpportunitySvm>, RestError>;
    async fn remove_opportunities(
        &self,
        permission_key: &PermissionKeySvm,
        chain_id: &ChainId,
        reason: OpportunityRemovalReason,
    ) -> anyhow::Result<()>;
    async fn remove_opportunity(
        &self,
        opportunity: &OpportunitySvm,
        reason: OpportunityRemovalReason,
    ) -> anyhow::Result<()>;
}
#[async_trait]
impl Database for DB {
    #[instrument(
        target = "metrics",
        name = "db_add_opportunity",
        fields(
            category = "db_queries",
            result = "success",
            name = "add_opportunity",
            tracing_enabled
        ),
        skip_all
    )]
    async fn add_opportunity(&self, opportunity: &OpportunitySvm) -> Result<(), RestError> {
        let metadata = opportunity.get_models_metadata();
        let chain_type = OpportunityMetadataSvm::get_chain_type(); // todo: remove?
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
            .await.map_err(|e| {
                tracing::Span::current().record("result", "error");
                tracing::error!("DB: Failed to insert opportunity: {}", e);
                RestError::TemporarilyUnavailable
            })?;
        Ok(())
    }

    #[instrument(
        target = "metrics",
        name = "db_get_opportunities",
        fields(
            category = "db_queries",
            result = "success",
            name = "get_opportunities",
            tracing_enabled
        ),
        skip_all
    )]
    async fn get_opportunities(
        &self,
        chain_id: ChainId,
        from_time: Option<OffsetDateTime>,
    ) -> Result<Vec<OpportunitySvm>, RestError> {
        let mut query = QueryBuilder::new("SELECT * from opportunity WHERE chain_type = ");
        query.push_bind(OpportunityMetadataSvm::get_chain_type());
        query.push(" AND chain_id = ");
        query.push_bind(chain_id.clone());
        if let Some(from_time) = from_time {
            query.push(" AND creation_time >= ");
            query.push_bind(from_time);
        }
        query.push(" ORDER BY creation_time ASC LIMIT ");
        query.push_bind(super::OPPORTUNITY_PAGE_SIZE_CAP as i64);
        let opps: Vec<models::Opportunity<OpportunityMetadataSvm>> =
            query.build_query_as().fetch_all(self).await.map_err(|e| {
                tracing::Span::current().record("result", "error");
                tracing::error!(
                    "DB: Failed to fetch opportunities: {} - chain_id: {:?} - from_time: {:?}",
                    e,
                    chain_id,
                    from_time,
                );
                RestError::TemporarilyUnavailable
            })?;

        opps.into_iter().map(|opp| opp.clone().try_into().map_err(
            |_| {
                tracing::error!(
                    "Failed to convert database opportunity to entity opportunity: {:?} - chain_id: {:?} - from_time: {:?}",
                    opp,
                    chain_id,
                    from_time,
                );
                RestError::TemporarilyUnavailable
            }
        )).collect()
    }

    #[instrument(
        target = "metrics",
        name = "db_remove_opportunities",
        fields(
            category = "db_queries",
            result = "success",
            name = "remove_opportunities",
            tracing_enabled
        ),
        skip_all
    )]
    async fn remove_opportunities(
        &self,
        permission_key: &PermissionKeySvm,
        chain_id: &ChainId,
        reason: OpportunityRemovalReason,
    ) -> anyhow::Result<()> {
        let now = OffsetDateTime::now_utc();
        sqlx::query("UPDATE opportunity SET removal_time = $1, removal_reason = $2 WHERE permission_key = $3 AND chain_id = $4 and removal_time IS NULL")
            .bind(PrimitiveDateTime::new(now.date(), now.time()))
            .bind(reason)
            .bind(permission_key.as_ref())
            .bind(chain_id)
            .execute(self)
            .await
            .inspect_err(|_| {
                tracing::Span::current().record("result", "error");
            })?;
        Ok(())
    }

    #[instrument(
        target = "metrics",
        name = "db_remove_opportunity",
        fields(
            category = "db_queries",
            result = "success",
            name = "remove_opportunity",
            tracing_enabled
        ),
        skip_all
    )]
    async fn remove_opportunity(
        &self,
        opportunity: &OpportunitySvm,
        reason: OpportunityRemovalReason,
    ) -> anyhow::Result<()> {
        let now = OffsetDateTime::now_utc();
        sqlx::query("UPDATE opportunity SET removal_time = $1, removal_reason = $2 WHERE id = $3 AND removal_time IS NULL")
            .bind(PrimitiveDateTime::new(now.date(), now.time()))
            .bind(reason)
            .bind(opportunity.id)
            .execute(self)
            .await
            .inspect_err(|_| {
                tracing::Span::current().record("result", "error");
            })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use {
        crate::{
            kernel::entities::PermissionKeySvm,
            opportunity::entities::{
                OpportunitySvm,
                OpportunitySvmProgram,
                OpportunitySvmProgramSwap,
                TokenAmountSvm,
            },
        },
        solana_sdk::pubkey::Pubkey,
        time::OffsetDateTime,
    };

    #[test]
    fn test_svm_program_metadata_json_roundtrip() {
        let op = OpportunitySvm {
            id:                 Default::default(),
            permission_key:     PermissionKeySvm([1; 65]),
            chain_id:           "".to_string(),
            sell_tokens:        vec![TokenAmountSvm {
                token:  Pubkey::new_unique(),
                amount: 2,
            }],
            buy_tokens:         vec![TokenAmountSvm {
                token:  Pubkey::new_unique(),
                amount: 1,
            }],
            creation_time:      OffsetDateTime::now_utc(),
            refresh_time:       OffsetDateTime::now_utc(),
            router:             Default::default(),
            permission_account: Default::default(),
            program:            OpportunitySvmProgram::Swap(
                OpportunitySvmProgramSwap::default_test_with_user_wallet_address(
                    Pubkey::new_unique(),
                ),
            ),
        };

        let metadata = op.get_models_metadata();
        let json = serde_json::to_string(&metadata).unwrap();
        assert!(!json.contains("memo"));

        let metadata_2 = serde_json::from_str(&json).unwrap();
        assert_eq!(metadata, metadata_2);

        let mut json = serde_json::to_value(&metadata).unwrap();
        json.as_object_mut()
            .unwrap()
            .insert("memo".to_string(), serde_json::Value::Null);
        let metadata_3 = serde_json::from_value(json).unwrap();
        assert_eq!(metadata, metadata_3);
    }
}
