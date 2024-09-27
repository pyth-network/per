use {
    super::{
        opportunity::{
            Opportunity,
            OpportunityCoreFields,
        },
        token_amount_svm::TokenAmountSvm,
    },
    crate::{
        opportunity::{
            api,
            repository::{
                self,
            },
        },
        state::UnixTimestampMicros,
    },
    solana_sdk::{
        hash::Hash,
        pubkey::Pubkey,
    },
    std::ops::Deref,
    time::OffsetDateTime,
    uuid::Uuid,
};

#[derive(Debug, Clone, PartialEq)]
pub struct OpportunitySvm {
    pub core_fields: OpportunityCoreFields<TokenAmountSvm>,

    pub order:      Vec<u8>,
    pub router:     Pubkey,
    pub permission: Pubkey,
    pub block_hash: Hash,
}

impl Opportunity for OpportunitySvm {
    type TokenAmount = TokenAmountSvm;
    type ModelMetadata = repository::OpportunityMetadataSvm;
    type ApiOpportunityCreate = api::OpportunityCreateSvm;
}

impl Deref for OpportunitySvm {
    type Target = OpportunityCoreFields<TokenAmountSvm>;

    fn deref(&self) -> &Self::Target {
        &self.core_fields
    }
}

// Model conversions
impl From<OpportunitySvm> for repository::OpportunityMetadataSvm {
    fn from(metadata: OpportunitySvm) -> Self {
        Self {
            order:      metadata.order,
            router:     metadata.router,
            permission: metadata.permission,
            block_hash: metadata.block_hash,
        }
    }
}

// API conversions
impl From<OpportunitySvm> for api::Opportunity {
    fn from(val: OpportunitySvm) -> Self {
        api::Opportunity::Svm(val.into())
    }
}

impl From<OpportunitySvm> for api::OpportunitySvm {
    fn from(val: OpportunitySvm) -> Self {
        api::OpportunitySvm {
            opportunity_id: val.id,
            creation_time:  val.creation_time,
            params:         api::OpportunityParamsSvm::V1(api::OpportunityParamsV1Svm::Kamino(
                api::OpportunityParamsV1KaminoSvm {
                    order:    val.order.clone(),
                    chain_id: val.chain_id.clone(),
                },
            )),
        }
    }
}

impl From<api::OpportunityCreateSvm> for OpportunitySvm {
    fn from(val: api::OpportunityCreateSvm) -> Self {
        let params = match val {
            api::OpportunityCreateSvm::V1(api::OpportunityCreateV1Svm::Kamino(val)) => val,
        };
        let id = Uuid::new_v4();
        let now_odt = OffsetDateTime::now_utc();
        OpportunitySvm {
            core_fields: OpportunityCoreFields::<TokenAmountSvm> {
                id,
                permission_key: [params.permission.to_bytes(), params.router.to_bytes()]
                    .concat()
                    .into(),
                chain_id: params.chain_id,
                sell_tokens: params.sell_tokens.into_iter().map(|t| t.into()).collect(),
                buy_tokens: params.buy_tokens.into_iter().map(|t| t.into()).collect(),
                creation_time: now_odt.unix_timestamp_nanos() / 1000 as UnixTimestampMicros,
            },
            order:       params.order,
            permission:  params.permission,
            router:      params.router,
            block_hash:  params.block_hash,
        }
    }
}
