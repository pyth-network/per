use {
    super::{
        opportunity::{
            Opportunity,
            OpportunityCoreFields,
        },
        token_amount_evm::TokenAmountEvm,
        OpportunityCoreFieldsCreate,
        OpportunityCreate,
    },
    crate::{
        opportunity::{
            api,
            repository::{
                self,
            },
        },
        state::{
            PermissionKey,
            UnixTimestampMicros,
        },
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

#[derive(Debug, Clone)]
pub struct OpportunityCreateEvm {
    pub core_fields: OpportunityCoreFieldsCreate<TokenAmountEvm>,

    pub target_contract:   ethers::abi::Address,
    pub target_calldata:   Bytes,
    pub target_call_value: U256,
}

impl Opportunity for OpportunityEvm {
    type TokenAmount = TokenAmountEvm;
    type ModelMetadata = repository::OpportunityMetadataEvm;
    type OpportunityCreate = OpportunityCreateEvm;
}

impl OpportunityCreate for OpportunityCreateEvm {
    type ApiOpportunityCreate = api::OpportunityCreateEvm;

    fn permission_key(&self) -> crate::kernel::entities::PermissionKey {
        self.core_fields.permission_key.clone()
    }
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

impl From<api::OpportunityCreateEvm> for OpportunityCreateEvm {
    fn from(val: api::OpportunityCreateEvm) -> Self {
        // let id = Uuid::new_v4();
        // let now_odt = OffsetDateTime::now_utc();
        let params = match val {
            api::OpportunityCreateEvm::V1(params) => params,
        };
        OpportunityCreateEvm {
            core_fields:       OpportunityCoreFieldsCreate::<TokenAmountEvm> {
                // id,
                permission_key: params.permission_key.clone(),
                chain_id:       params.chain_id.clone(),
                sell_tokens:    params.sell_tokens.into_iter().map(|t| t.into()).collect(),
                buy_tokens:     params.buy_tokens.into_iter().map(|t| t.into()).collect(),
                // creation_time: now_odt.unix_timestamp_nanos() / 1000 as UnixTimestampMicros,
            },
            target_contract:   params.target_contract,
            target_calldata:   params.target_calldata,
            target_call_value: params.target_call_value,
        }
    }
}

impl TryFrom<repository::Opportunity<repository::OpportunityMetadataEvm>> for OpportunityEvm {
    type Error = anyhow::Error;

    fn try_from(
        val: repository::Opportunity<repository::OpportunityMetadataEvm>,
    ) -> Result<Self, Self::Error> {
        let sell_tokens = serde_json::from_value(val.sell_tokens.clone()).map_err(|e| {
            tracing::error!(
                "Failed to deserialize sell_tokens for database opportunity evm: {:?} - {}",
                val,
                e
            );
            anyhow::anyhow!(e)
        })?;
        let buy_tokens = serde_json::from_value(val.buy_tokens.clone()).map_err(|e| {
            tracing::error!(
                "Failed to deserialize buy_tokens for database opportunity evm: {:?} - {}",
                val,
                e
            );
            anyhow::anyhow!(e)
        })?;
        Ok(OpportunityEvm {
            core_fields:       OpportunityCoreFields {
                id: val.id,
                creation_time: val.creation_time.assume_utc().unix_timestamp_nanos(),
                permission_key: PermissionKey::from(val.permission_key),
                chain_id: val.chain_id,
                sell_tokens,
                buy_tokens,
            },
            target_contract:   val.metadata.target_contract,
            target_call_value: val.metadata.target_call_value,
            target_calldata:   val.metadata.target_calldata.clone(),
        })
    }
}

impl From<OpportunityCreateEvm> for OpportunityEvm {
    fn from(val: OpportunityCreateEvm) -> Self {
        let id = Uuid::new_v4();
        let odt = OffsetDateTime::now_utc();
        OpportunityEvm {
            core_fields:       OpportunityCoreFields::<TokenAmountEvm> {
                id,
                creation_time: odt.unix_timestamp_nanos() / 1000 as UnixTimestampMicros,
                permission_key: val.core_fields.permission_key.clone(),
                chain_id: val.core_fields.chain_id.clone(),
                sell_tokens: val.core_fields.sell_tokens.clone(),
                buy_tokens: val.core_fields.buy_tokens.clone(),
            },
            target_contract:   val.target_contract,
            target_call_value: val.target_call_value,
            target_calldata:   val.target_calldata.clone(),
        }
    }
}

impl From<OpportunityEvm> for OpportunityCreateEvm {
    fn from(val: OpportunityEvm) -> Self {
        OpportunityCreateEvm {
            core_fields:       OpportunityCoreFieldsCreate::<TokenAmountEvm> {
                permission_key: val.core_fields.permission_key.clone(),
                chain_id:       val.core_fields.chain_id.clone(),
                sell_tokens:    val.core_fields.sell_tokens.clone(),
                buy_tokens:     val.core_fields.buy_tokens.clone(),
            },
            target_contract:   val.target_contract,
            target_call_value: val.target_call_value,
            target_calldata:   val.target_calldata.clone(),
        }
    }
}

impl PartialEq<OpportunityCreateEvm> for OpportunityEvm {
    fn eq(&self, other: &OpportunityCreateEvm) -> bool {
        self.target_contract == other.target_contract
            && self.target_call_value == other.target_call_value
            && self.target_calldata == other.target_calldata
            && self.core_fields.buy_tokens == other.core_fields.buy_tokens
            && self.core_fields.sell_tokens == other.core_fields.sell_tokens
            && self.core_fields.chain_id == other.core_fields.chain_id
            && self.core_fields.permission_key == other.core_fields.permission_key
    }
}
