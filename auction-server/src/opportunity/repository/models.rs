#[cfg(test)]
use mockall::automock;
use {
    super::models,
    crate::{
        api::RestError,
        kernel::{
            db::{
                DBAnalytics,
                DB,
            },
            entities::{
                ChainId,
                PermissionKeySvm,
            },
        },
        models::{
            ChainType,
            ProfileId,
        },
        opportunity::{
            entities,
            entities::{
                FeeToken,
                OpportunitySvm,
                TokenAccountInitializationConfigs,
            },
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

#[derive(Clone, Debug, PartialEq, PartialOrd, sqlx::Type, Serialize, Deserialize)]
#[sqlx(type_name = "opportunity_removal_reason", rename_all = "lowercase")]
#[serde(rename_all = "snake_case")]
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
    #[serde(default = "default_fee_ppm")]
    pub referral_fee_ppm:                     u64,
    pub referral_fee_bps:                     u16,
    #[serde(default = "default_fee_ppm")]
    pub platform_fee_ppm:                     u64,
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

fn default_fee_ppm() -> u64 {
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
    pub profile_id:     Option<ProfileId>,
}


#[cfg_attr(test, automock)]
#[async_trait]
pub trait DatabaseAnalytics: Send + Sync + 'static {
    async fn add_opportunity(
        &self,
        opportunity: &OpportunitySvm,
        removal_time: Option<OffsetDateTime>,
        removal_reason: Option<OpportunityRemovalReason>,
    ) -> Result<(), RestError>;
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

#[derive(Debug, Serialize, Deserialize, clickhouse::Row)]
#[serde_as]
pub struct OpportunityAnalytics {
    pub id:             Uuid,
    pub creation_time:  OffsetDateTime,
    pub permission_key: String,
    pub chain_id:       String,

    pub program: String,

    pub sell_tokens: JsonValue,
    pub buy_tokens:  JsonValue,

    #[serde_as(as = "DisplayFromStr")]
    pub sell_token_mint:      Pubkey,
    pub sell_token_amount:    u64,
    pub sell_token_usd_price: Option<f64>,

    #[serde_as(as = "DisplayFromStr")]
    pub buy_token_mint:      Pubkey,
    pub buy_token_amount:    u64,
    pub buy_token_usd_price: Option<f64>,

    pub removal_time:   Option<OffsetDateTime>,
    pub removal_reason: Option<OpportunityRemovalReason>,

    // Limo-specific
    #[serde_as(as = "DisplayFromStr")]
    pub limo_order:         Option<Pubkey>,
    #[serde_as(as = "DisplayFromStr")]
    pub limo_order_address: Option<Pubkey>,
    pub limo_slot:          Option<u64>,

    // Swap-specific
    #[serde_as(as = "DisplayFromStr")]
    pub swap_user_wallet_address:                  Option<Pubkey>,
    pub swap_fee_token:                            Option<FeeToken>,
    pub swap_referral_fee_bps:                     Option<u16>,
    pub swap_referral_fee_ppm:                     Option<u64>,
    pub swap_platform_fee_bps:                     Option<u64>,
    pub swap_platform_fee_ppm:                     Option<u64>,
    pub swap_token_program_user:                   Option<Pubkey>,
    pub swap_token_program_searcher:               Option<Pubkey>,
    pub swap_token_account_initialization_configs: Option<JsonValue>,
    pub swap_user_mint_user_balance:               Option<u64>,
    pub swap_memo:                                 Option<String>,
    pub swap_cancellable:                          Option<bool>,
    pub swap_minimum_lifetime:                     Option<u32>,

    pub profile_id: Option<Uuid>,
}

#[async_trait]
impl DatabaseAnalytics for DBAnalytics {
    #[instrument(
        target = "metrics",
        name = "db_analytics_add_opportunity",
        fields(
            category = "db_analytics_queries",
            result = "success",
            name = "add_opportunity",
            tracing_enabled
        ),
        skip_all
    )]
    async fn add_opportunity(
        &self,
        opportunity: &OpportunitySvm,
        removal_time: Option<OffsetDateTime>,
        removal_reason: Option<OpportunityRemovalReason>,
    ) -> Result<(), RestError> {
        let sell_token = opportunity.sell_tokens.first().ok_or_else(|| {
            tracing::error!(opportunity = ?opportunity, "Opportunity has no sell tokens");
            RestError::TemporarilyUnavailable
        })?;

        let buy_token = opportunity.buy_tokens.first().ok_or_else(|| {
            tracing::error!(opportunity = ?opportunity, "Opportunity has no buy tokens");
            RestError::TemporarilyUnavailable
        })?;

        let opportunity_analytics: OpportunityAnalytics = match opportunity.program.clone() {
            entities::OpportunitySvmProgram::Limo(params) => OpportunityAnalytics {
                id: opportunity.id,
                creation_time: opportunity.creation_time,
                permission_key: format!("{:?}", opportunity.permission_key),
                chain_id: opportunity.chain_id.clone(),
                program: "limo".to_string(),
                sell_tokens: serde_json::to_value(opportunity.sell_tokens.clone())
                    .expect("Failed to deserialize sell tokens"),
                buy_tokens: serde_json::to_value(opportunity.buy_tokens.clone())
                    .expect("Failed to deserialize buy tokens"),
                removal_time,
                removal_reason,
                sell_token_mint: sell_token.token,
                sell_token_amount: sell_token.amount,
                sell_token_usd_price: None,
                buy_token_mint: buy_token.token,
                buy_token_amount: buy_token.amount,
                buy_token_usd_price: None,
                limo_order: Some(params.order_address),
                limo_order_address: Some(params.order_address),
                limo_slot: Some(params.slot),
                swap_user_wallet_address: None,
                swap_fee_token: None,
                swap_referral_fee_bps: None,
                swap_referral_fee_ppm: None,
                swap_platform_fee_bps: None,
                swap_platform_fee_ppm: None,
                swap_token_program_user: None,
                swap_token_program_searcher: None,
                swap_user_mint_user_balance: None,
                swap_token_account_initialization_configs: None,
                swap_memo: None,
                swap_cancellable: None,
                swap_minimum_lifetime: None,

                profile_id: opportunity.profile_id,
            },
            entities::OpportunitySvmProgram::Swap(params) => {
                OpportunityAnalytics {
                    id: opportunity.id,
                    creation_time: opportunity.creation_time,
                    permission_key: format!("{:?}", opportunity.permission_key),
                    chain_id: opportunity.chain_id.clone(),
                    program: "limo".to_string(),
                    sell_tokens: serde_json::to_value(opportunity.sell_tokens.clone())
                        .expect("Failed to deserialize sell tokens"),
                    buy_tokens: serde_json::to_value(opportunity.buy_tokens.clone())
                        .expect("Failed to deserialize buy tokens"),
                    removal_time,
                    removal_reason,
                    sell_token_mint: sell_token.token,
                    sell_token_amount: sell_token.amount,
                    sell_token_usd_price: None,
                    buy_token_mint: buy_token.token,
                    buy_token_amount: buy_token.amount,
                    buy_token_usd_price: None,
                    limo_order: None,
                    limo_order_address: None,
                    limo_slot: None,
                    swap_user_wallet_address: Some(params.user_wallet_address),
                    // swap_fee_token: Some(params.fee_token),
                    swap_fee_token: None,
                    swap_referral_fee_bps: Some(params.referral_fee_bps),
                    swap_referral_fee_ppm: Some(params.referral_fee_ppm),
                    swap_platform_fee_bps: Some(params.platform_fee_bps),
                    swap_platform_fee_ppm: Some(params.platform_fee_ppm),
                    swap_token_program_user: Some(params.token_program_user),
                    swap_token_program_searcher: Some(params.token_program_searcher),
                    swap_user_mint_user_balance: Some(params.user_mint_user_balance),
                    swap_token_account_initialization_configs: Some(
                        serde_json::to_value(params.token_account_initialization_configs)
                            .expect("Failed to serialize token account initialization configs"),
                    ),
                    swap_memo: params.memo,
                    swap_cancellable: Some(params.cancellable),
                    swap_minimum_lifetime: params.minimum_lifetime,

                    profile_id: opportunity.profile_id,
                }
            }
        };
        let mut insert = self.insert("opportunity").map_err(|err| {
            tracing::error!(error = ?err, "Failed to insert analytics opportunity");
            RestError::TemporarilyUnavailable
        })?;
        insert.write(&opportunity_analytics).await.map_err(|err| {
            tracing::error!(error = ?err, "Failed to write to analytics opportunity");
            RestError::TemporarilyUnavailable
        })?;
        insert.end().await.map_err(|err| {
            tracing::error!(error = ?err, "Failed to end write to analytics opportunity");
            RestError::TemporarilyUnavailable
        })
    }
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
                                                        buy_tokens,
                                                        profile_id) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
        opportunity.id,
        PrimitiveDateTime::new(opportunity.creation_time.date(), opportunity.creation_time.time()),
        opportunity.permission_key.to_vec(),
        opportunity.chain_id,
        chain_type as _,
        serde_json::to_value(metadata).expect("Failed to serialize metadata"),
        serde_json::to_value(&opportunity.sell_tokens).expect("Failed to serialize sell_tokens"),
        serde_json::to_value(&opportunity.buy_tokens).expect("Failed to serialize buy_tokens"),
        opportunity.profile_id)
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
            profile_id:         None,
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
