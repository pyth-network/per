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
    },
    solana_sdk::{
        clock::Slot,
        pubkey::Pubkey,
    },
    std::ops::Deref,
};

#[derive(Debug, Clone, PartialEq)]
pub struct OpportunitySvmProgramLimo {
    pub order:         Vec<u8>,
    pub order_address: Pubkey,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OpportunitySvmProgramWallet {
    pub user_wallet_address:         Pubkey,
    pub maximum_slippage_percentage: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum OpportunitySvmProgram {
    Limo(OpportunitySvmProgramLimo),
    Phantom(OpportunitySvmProgramWallet),
}

#[derive(Debug, Clone, PartialEq)]
pub struct OpportunitySvm {
    pub core_fields: OpportunityCoreFields<TokenAmountSvm>,

    pub router:             Pubkey,
    pub permission_account: Pubkey,
    pub program:            OpportunitySvmProgram,
    pub slot:               Slot,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OpportunityCreateSvm {
    pub core_fields: OpportunityCoreFieldsCreate<TokenAmountSvm>,

    pub router:             Pubkey,
    pub permission_account: Pubkey,
    pub program:            OpportunitySvmProgram,
    pub slot:               Slot,
}

impl Opportunity for OpportunitySvm {
    type TokenAmount = TokenAmountSvm;
    type ModelMetadata = repository::OpportunityMetadataSvm;
    type OpportunityCreate = OpportunityCreateSvm;

    fn new_with_current_time(val: Self::OpportunityCreate) -> Self {
        OpportunitySvm {
            core_fields:        OpportunityCoreFields::<TokenAmountSvm>::new_with_current_time(
                val.core_fields,
            ),
            router:             val.router,
            permission_account: val.permission_account,
            program:            val.program,
            slot:               val.slot,
        }
    }

    fn get_models_metadata(&self) -> Self::ModelMetadata {
        let program = match self.program.clone() {
            OpportunitySvmProgram::Limo(program) => {
                repository::OpportunityMetadataSvmProgram::Limo(
                    repository::OpportunityMetadataSvmProgramLimo {
                        order:         program.order,
                        order_address: program.order_address,
                    },
                )
            }
            OpportunitySvmProgram::Phantom(program) => {
                repository::OpportunityMetadataSvmProgram::Phantom(
                    repository::OpportunityMetadataSvmProgramWallet {
                        user_wallet_address:         program.user_wallet_address,
                        maximum_slippage_percentage: program.maximum_slippage_percentage,
                    },
                )
            }
        };
        Self::ModelMetadata {
            program,
            router: self.router,
            permission_account: self.permission_account,
            slot: self.slot,
        }
    }
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

impl From<OpportunitySvm> for api::Opportunity {
    fn from(val: OpportunitySvm) -> Self {
        api::Opportunity::Svm(val.into())
    }
}

impl From<OpportunitySvm> for api::OpportunitySvm {
    fn from(val: OpportunitySvm) -> Self {
        let program = match val.program.clone() {
            OpportunitySvmProgram::Limo(prgoram) => api::OpportunityParamsV1ProgramSvm::Limo {
                order:         prgoram.order,
                order_address: prgoram.order_address,
            },
            OpportunitySvmProgram::Phantom(program) => {
                api::OpportunityParamsV1ProgramSvm::Phantom {
                    user_wallet_address:         program.user_wallet_address,
                    maximum_slippage_percentage: program.maximum_slippage_percentage,
                    permission_account:          val.permission_account,
                    router_account:              val.router,
                    // TODO can we make it type safe?
                    sell_token:                  val
                        .sell_tokens
                        .first()
                        .map(|t| t.clone().into())
                        .ok_or(anyhow::anyhow!(
                            "Failed to get sell token from opportunity svm"
                        ))
                        .expect("Failed to get sell token from opportunity svm"),
                    buy_token:                   val
                        .sell_tokens
                        .first()
                        .map(|t| t.clone().into())
                        .ok_or(anyhow::anyhow!(
                            "Failed to get sell token from opportunity svm"
                        ))
                        .expect("Failed to get sell token from opportunity svm"),
                }
            }
        };
        api::OpportunitySvm {
            opportunity_id: val.id,
            creation_time:  val.creation_time,
            slot:           val.slot,
            params:         api::OpportunityParamsSvm::V1(api::OpportunityParamsV1Svm {
                program,
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
        let program = match val.metadata.program.clone() {
            repository::OpportunityMetadataSvmProgram::Limo(program) => {
                OpportunitySvmProgram::Limo(OpportunitySvmProgramLimo {
                    order:         program.order,
                    order_address: program.order_address,
                })
            }
            repository::OpportunityMetadataSvmProgram::Phantom(program) => {
                OpportunitySvmProgram::Phantom(OpportunitySvmProgramWallet {
                    user_wallet_address:         program.user_wallet_address,
                    maximum_slippage_percentage: program.maximum_slippage_percentage,
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
            permission_account: val.metadata.permission_account,
            program,
            slot: val.metadata.slot,
        })
    }
}

impl From<api::OpportunityCreateSvm> for OpportunityCreateSvm {
    fn from(val: api::OpportunityCreateSvm) -> Self {
        let api::OpportunityCreateSvm::V1(params) = val;
        let program = match params.program_params {
            api::OpportunityCreateProgramParamsV1Svm::Limo {
                order,
                order_address,
            } => OpportunitySvmProgram::Limo(OpportunitySvmProgramLimo {
                order,
                order_address,
            }),
            api::OpportunityCreateProgramParamsV1Svm::Phantom {
                user_wallet_address,
                maximum_slippage_percentage,
            } => OpportunitySvmProgram::Phantom(OpportunitySvmProgramWallet {
                user_wallet_address,
                maximum_slippage_percentage,
            }),
        };

        OpportunityCreateSvm {
            core_fields: OpportunityCoreFieldsCreate::<TokenAmountSvm> {
                permission_key: [
                    params.router.to_bytes(),
                    params.permission_account.to_bytes(),
                ]
                .concat()
                .into(),
                chain_id:       params.chain_id,
                sell_tokens:    params.sell_tokens.into_iter().map(|t| t.into()).collect(),
                buy_tokens:     params.buy_tokens.into_iter().map(|t| t.into()).collect(),
            },
            program,
            permission_account: params.permission_account,
            router: params.router,
            slot: params.slot,
        }
    }
}

impl From<OpportunitySvm> for OpportunityCreateSvm {
    fn from(val: OpportunitySvm) -> Self {
        OpportunityCreateSvm {
            core_fields:        OpportunityCoreFieldsCreate::<TokenAmountSvm> {
                permission_key: val.core_fields.permission_key,
                chain_id:       val.core_fields.chain_id,
                sell_tokens:    val.core_fields.sell_tokens,
                buy_tokens:     val.core_fields.buy_tokens,
            },
            router:             val.router,
            permission_account: val.permission_account,
            program:            val.program,
            slot:               val.slot,
        }
    }
}

impl OpportunitySvm {
    pub fn get_missing_signers(&self) -> Vec<Pubkey> {
        match self.program.clone() {
            OpportunitySvmProgram::Phantom(data) => vec![data.user_wallet_address],
            OpportunitySvmProgram::Limo(_) => vec![],
        }
    }
}
