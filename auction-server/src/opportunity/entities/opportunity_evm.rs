use {
    super::{
        opportunity::{
            Opportunity,
            OpportunityCoreFields,
        },
        token_amount_evm::TokenAmountEvm,
        OpportunityComparison,
        OpportunityCoreFieldsCreate,
        OpportunityCreate,
    },
    crate::{
        kernel::entities::PermissionKey,
        opportunity::repository,
    },
    ethers::types::{
        Bytes,
        U256,
    },
    express_relay_api_types::opportunity as api,
    std::ops::Deref,
    time::OffsetDateTime,
};

// TODO revise the entities for opportunity, Maybe generic opportunity with params
#[derive(Debug, Clone, PartialEq)]
pub struct OpportunityEvm {
    pub core_fields: OpportunityCoreFields<TokenAmountEvm>,

    pub target_contract:   ethers::abi::Address,
    pub target_calldata:   Bytes,
    pub target_call_value: U256,
}

#[derive(Debug, Clone, PartialEq)]
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

    fn new_with_current_time(val: Self::OpportunityCreate) -> Self {
        Self {
            core_fields:       OpportunityCoreFields::new_with_current_time(val.core_fields),
            target_contract:   val.target_contract,
            target_call_value: val.target_call_value,
            target_calldata:   val.target_calldata,
        }
    }

    fn get_models_metadata(&self) -> Self::ModelMetadata {
        Self::ModelMetadata {
            target_contract:   self.target_contract,
            target_call_value: self.target_call_value,
            target_calldata:   self.target_calldata.clone(),
        }
    }

    fn get_opportunity_delete(&self) -> api::OpportunityDelete {
        api::OpportunityDelete::Evm(api::OpportunityDeleteEvm::V1(api::OpportunityDeleteV1Evm {
            permission_key: self.core_fields.permission_key.clone(),
            chain_id:       self.core_fields.chain_id.clone(),
        }))
    }

    fn compare(&self, other: &Self::OpportunityCreate) -> super::OpportunityComparison {
        if *other == self.clone().into() {
            OpportunityComparison::Duplicate
        } else {
            OpportunityComparison::New
        }
    }

    fn refresh(&mut self) {
        self.core_fields.refresh_time = OffsetDateTime::now_utc();
    }
}

impl OpportunityCreate for OpportunityCreateEvm {
    type ApiOpportunityCreate = api::OpportunityCreateEvm;

    fn get_key(&self) -> super::OpportunityKey {
        super::OpportunityKey(
            self.core_fields.chain_id.clone(),
            self.core_fields.permission_key.clone(),
        )
    }
}

impl Deref for OpportunityEvm {
    type Target = OpportunityCoreFields<TokenAmountEvm>;

    fn deref(&self) -> &Self::Target {
        &self.core_fields
    }
}

impl From<OpportunityEvm> for api::Opportunity {
    fn from(val: OpportunityEvm) -> Self {
        api::Opportunity::Evm(val.into())
    }
}

impl From<OpportunityEvm> for api::OpportunityEvm {
    fn from(val: OpportunityEvm) -> Self {
        api::OpportunityEvm {
            opportunity_id: val.id,
            creation_time:  val.creation_time.unix_timestamp_nanos() / 1000,
            params:         api::OpportunityParamsEvm::V1(api::OpportunityParamsV1Evm(
                api::OpportunityCreateV1Evm {
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
                },
            )),
        }
    }
}

impl From<api::OpportunityCreateEvm> for OpportunityCreateEvm {
    fn from(val: api::OpportunityCreateEvm) -> Self {
        let api::OpportunityCreateEvm::V1(params) = val;
        OpportunityCreateEvm {
            core_fields:       OpportunityCoreFieldsCreate::<TokenAmountEvm> {
                permission_key: params.permission_key,
                chain_id:       params.chain_id,
                sell_tokens:    params.sell_tokens.into_iter().map(|t| t.into()).collect(),
                buy_tokens:     params.buy_tokens.into_iter().map(|t| t.into()).collect(),
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
                creation_time: val.creation_time.assume_utc(),
                refresh_time: val.creation_time.assume_utc(),
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

impl From<OpportunityEvm> for OpportunityCreateEvm {
    fn from(val: OpportunityEvm) -> Self {
        OpportunityCreateEvm {
            core_fields:       OpportunityCoreFieldsCreate::<TokenAmountEvm> {
                permission_key: val.core_fields.permission_key,
                chain_id:       val.core_fields.chain_id,
                sell_tokens:    val.core_fields.sell_tokens,
                buy_tokens:     val.core_fields.buy_tokens,
            },
            target_contract:   val.target_contract,
            target_call_value: val.target_call_value,
            target_calldata:   val.target_calldata,
        }
    }
}
