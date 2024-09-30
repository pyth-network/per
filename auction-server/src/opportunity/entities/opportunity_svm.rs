use {
    super::{
        opportunity::{
            Opportunity,
            OpportunityCoreFields,
        },
        token_amount_svm::TokenAmountSvm,
        OpportunityCoreFieldsCreate,
        OpportunityCreate,
    },
    crate::{
        kernel::entities::PermissionKey,
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
pub struct OpportunitySvmClientKamino {
    pub order: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum OpportunitySvmClient {
    Kamino(OpportunitySvmClientKamino),
}

#[derive(Debug, Clone, PartialEq)]
pub struct OpportunitySvm {
    pub core_fields: OpportunityCoreFields<TokenAmountSvm>,

    pub router:     Pubkey,
    pub permission: Pubkey,
    pub block_hash: Hash,
    pub client:     OpportunitySvmClient,
}

#[derive(Debug, Clone)]
pub struct OpportunityCreateSvm {
    pub core_fields: OpportunityCoreFieldsCreate<TokenAmountSvm>,

    pub router:     Pubkey,
    pub permission: Pubkey,
    pub block_hash: Hash,
    pub client:     OpportunitySvmClient,
}

impl Opportunity for OpportunitySvm {
    type TokenAmount = TokenAmountSvm;
    type ModelMetadata = repository::OpportunityMetadataSvm;
    type OpportunityCreate = OpportunityCreateSvm;
}

impl OpportunityCreate for OpportunityCreateSvm {
    type ApiOpportunityCreate = api::OpportunityCreateSvm;

    fn permission_key(&self) -> crate::kernel::entities::PermissionKey {
        self.core_fields.permission_key.clone()
    }
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
        let client = match metadata.client {
            OpportunitySvmClient::Kamino(client) => {
                repository::OpportunityMetadataSvmClient::Kamino(
                    repository::OpportunityMetadataSvmClientKamino {
                        order: client.order,
                    },
                )
            }
        };
        Self {
            client,
            router: metadata.router,
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
        let client = match val.client.clone() {
            OpportunitySvmClient::Kamino(client) => {
                api::OpportunityParamsV1ClientSvm::Kamino(api::OpportunityParamsV1KaminoSvm {
                    order: client.order,
                })
            }
        };
        api::OpportunitySvm {
            opportunity_id: val.id.clone(),
            creation_time:  val.creation_time,
            params:         api::OpportunityParamsSvm::V1(api::OpportunityParamsV1Svm {
                client,
                chain_id: val.chain_id.clone(),
            }),
        }
    }
}


impl TryFrom<repository::Opportunity<repository::OpportunityMetadataSvm>> for OpportunitySvm {
    type Error = anyhow::Error;

    fn try_from(
        val: repository::Opportunity<repository::OpportunityMetadataSvm>,
    ) -> Result<Self, Self::Error> {
        let sell_tokens = serde_json::from_value(val.sell_tokens.clone()).map_err(|e| {
            tracing::error!(
                "Failed to deserialize sell_tokens for database opportunity svm: {:?} - {}",
                val,
                e
            );
            anyhow::anyhow!(e)
        })?;
        let buy_tokens = serde_json::from_value(val.buy_tokens.clone()).map_err(|e| {
            tracing::error!(
                "Failed to deserialize buy_tokens for database opportunity svm: {:?} - {}",
                val,
                e
            );
            anyhow::anyhow!(e)
        })?;
        let client = match val.metadata.client.clone() {
            repository::OpportunityMetadataSvmClient::Kamino(client) => {
                OpportunitySvmClient::Kamino(OpportunitySvmClientKamino {
                    order: client.order,
                })
            }
        };
        Ok(OpportunitySvm {
            core_fields: OpportunityCoreFields {
                id: val.id,
                creation_time: val.creation_time.assume_utc().unix_timestamp_nanos(),
                permission_key: PermissionKey::from(val.permission_key),
                chain_id: val.chain_id,
                sell_tokens,
                buy_tokens,
            },
            router: val.metadata.router,
            permission: val.metadata.permission,
            block_hash: val.metadata.block_hash,
            client,
        })
    }
}

impl From<api::OpportunityCreateSvm> for OpportunityCreateSvm {
    fn from(val: api::OpportunityCreateSvm) -> Self {
        let params = match val {
            api::OpportunityCreateSvm::V1(params) => params,
        };
        let client = match params.client_params {
            api::OpportunityCreateClientParamsV1Svm::Kamino(params) => {
                OpportunitySvmClient::Kamino(OpportunitySvmClientKamino {
                    order: params.order,
                })
            }
        };
        OpportunityCreateSvm {
            core_fields: OpportunityCoreFieldsCreate::<TokenAmountSvm> {
                permission_key: [params.permission.to_bytes(), params.router.to_bytes()]
                    .concat()
                    .into(),
                chain_id:       params.chain_id,
                sell_tokens:    params.sell_tokens.into_iter().map(|t| t.into()).collect(),
                buy_tokens:     params.buy_tokens.into_iter().map(|t| t.into()).collect(),
            },
            client,
            permission: params.permission,
            router: params.router,
            block_hash: params.block_hash,
        }
    }
}

impl From<OpportunityCreateSvm> for OpportunitySvm {
    fn from(val: OpportunityCreateSvm) -> Self {
        let id = Uuid::new_v4();
        let odt = OffsetDateTime::now_utc();
        OpportunitySvm {
            core_fields: OpportunityCoreFields::<TokenAmountSvm> {
                id,
                creation_time: odt.unix_timestamp_nanos() / 1000 as UnixTimestampMicros,
                permission_key: val.core_fields.permission_key.clone(),
                chain_id: val.core_fields.chain_id.clone(),
                sell_tokens: val.core_fields.sell_tokens.clone(),
                buy_tokens: val.core_fields.buy_tokens.clone(),
            },
            router:      val.router,
            permission:  val.permission,
            block_hash:  val.block_hash,
            client:      val.client,
        }
    }
}

impl From<OpportunitySvm> for OpportunityCreateSvm {
    fn from(val: OpportunitySvm) -> Self {
        OpportunityCreateSvm {
            core_fields: OpportunityCoreFieldsCreate::<TokenAmountSvm> {
                permission_key: val.core_fields.permission_key.clone(),
                chain_id:       val.core_fields.chain_id.clone(),
                sell_tokens:    val.core_fields.sell_tokens.clone(),
                buy_tokens:     val.core_fields.buy_tokens.clone(),
            },
            router:      val.router,
            permission:  val.permission,
            block_hash:  val.block_hash,
            client:      val.client,
        }
    }
}

impl PartialEq<OpportunityCreateSvm> for OpportunitySvm {
    fn eq(&self, other: &OpportunityCreateSvm) -> bool {
        self.router == other.router
            && self.permission == other.permission
            && self.block_hash == other.block_hash
            && self.client == other.client
            && self.core_fields.buy_tokens == other.core_fields.buy_tokens
            && self.core_fields.sell_tokens == other.core_fields.sell_tokens
            && self.core_fields.chain_id == other.core_fields.chain_id
            && self.core_fields.permission_key == other.core_fields.permission_key
    }
}
