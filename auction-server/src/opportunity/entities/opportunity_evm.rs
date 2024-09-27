use {
    super::{
        opportunity::{
            Opportunity,
            OpportunityCoreFields,
        },
        token_amount_evm::TokenAmountEvm,
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
    ethers::types::{
        Bytes,
        U256,
    },
    std::ops::Deref,
    time::OffsetDateTime,
    uuid::Uuid,
};


#[derive(Debug, Clone, PartialEq)]
pub struct OpportunityEvm {
    pub core_fields: OpportunityCoreFields<TokenAmountEvm>,

    pub target_contract:   ethers::abi::Address,
    pub target_calldata:   Bytes,
    pub target_call_value: U256,
}

impl Opportunity for OpportunityEvm {
    type TokenAmount = TokenAmountEvm;
    type ModelMetadata = repository::OpportunityMetadataEvm;
    type ApiOpportunityCreate = api::OpportunityCreateEvm;
}

impl Deref for OpportunityEvm {
    type Target = OpportunityCoreFields<TokenAmountEvm>;

    fn deref(&self) -> &Self::Target {
        &self.core_fields
    }
}

// Model conversions
impl From<OpportunityEvm> for repository::OpportunityMetadataEvm {
    fn from(metadata: OpportunityEvm) -> Self {
        Self {
            target_contract:   metadata.target_contract,
            target_call_value: metadata.target_call_value,
            target_calldata:   metadata.target_calldata,
        }
    }
}

// API conversions
impl From<OpportunityEvm> for api::Opportunity {
    fn from(val: OpportunityEvm) -> Self {
        api::Opportunity::Evm(val.into())
    }
}

impl From<OpportunityEvm> for api::OpportunityEvm {
    fn from(val: OpportunityEvm) -> Self {
        api::OpportunityEvm {
            opportunity_id: val.id,
            creation_time:  val.creation_time,
            params:         api::OpportunityCreateEvm::V1(api::OpportunityCreateV1Evm {
                permission_key:    val.permission_key.clone(),
                chain_id:          val.chain_id.clone(),
                target_contract:   val.target_contract,
                target_calldata:   val.target_calldata.clone(),
                target_call_value: val.target_call_value,
                sell_tokens:       val
                    .sell_tokens
                    .clone()
                    .into_iter()
                    .map(|t| t.into())
                    .collect(),
                buy_tokens:        val
                    .buy_tokens
                    .clone()
                    .into_iter()
                    .map(|t| t.into())
                    .collect(),
            }),
        }
    }
}

impl From<api::OpportunityCreateEvm> for OpportunityEvm {
    fn from(val: api::OpportunityCreateEvm) -> Self {
        let id = Uuid::new_v4();
        let now_odt = OffsetDateTime::now_utc();
        let params = match val {
            api::OpportunityCreateEvm::V1(params) => params,
        };
        OpportunityEvm {
            core_fields:       OpportunityCoreFields::<TokenAmountEvm> {
                id,
                permission_key: params.permission_key.clone(),
                chain_id: params.chain_id.clone(),
                sell_tokens: params.sell_tokens.into_iter().map(|t| t.into()).collect(),
                buy_tokens: params.buy_tokens.into_iter().map(|t| t.into()).collect(),
                creation_time: now_odt.unix_timestamp_nanos() / 1000 as UnixTimestampMicros,
            },
            target_contract:   params.target_contract,
            target_calldata:   params.target_calldata,
            target_call_value: params.target_call_value,
        }
    }
}
