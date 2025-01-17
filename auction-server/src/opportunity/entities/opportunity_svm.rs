use {
    super::{
        opportunity::{
            Opportunity,
            OpportunityCoreFields,
        },
        token_amount_svm::TokenAmountSvm,
        OpportunityComparison,
        OpportunityCoreFieldsCreate,
        OpportunityCreate,
    },
    crate::{
        auction::entities::BidPaymentInstructionType,
        kernel::entities::PermissionKey,
        opportunity::{
            entities::QuoteTokens,
            repository,
        },
    },
    ::express_relay::FeeToken as ProgramFeeToken,
    express_relay_api_types::opportunity as api,
    serde::{
        Deserialize,
        Serialize,
    },
    solana_sdk::{
        clock::Slot,
        pubkey::Pubkey,
    },
    std::ops::Deref,
    time::{
        Duration,
        OffsetDateTime,
    },
};

#[derive(Debug, Clone, PartialEq)]
pub struct OpportunitySvmProgramLimo {
    pub order:         Vec<u8>,
    pub order_address: Pubkey,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeeToken {
    InputToken,
    OutputToken,
}

impl PartialEq<ProgramFeeToken> for FeeToken {
    fn eq(&self, other: &ProgramFeeToken) -> bool {
        match self {
            FeeToken::InputToken => matches!(other, ProgramFeeToken::Input),
            FeeToken::OutputToken => matches!(other, ProgramFeeToken::Output),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct OpportunitySvmProgramSwap {
    pub user_wallet_address:  Pubkey,
    pub fee_token:            FeeToken,
    pub referral_fee_bps:     u16,
    // TODO*: these really should not live here. they should live in the opportunity core fields, but we don't want to introduce a breaking change. in any case, the need for the token programs is another sign that quotes should be separated from the traditional opportunity struct.
    pub input_token_program:  Pubkey,
    pub output_token_program: Pubkey,
}

#[derive(Debug, Clone, PartialEq)]
pub enum OpportunitySvmProgram {
    Limo(OpportunitySvmProgramLimo),
    Swap(OpportunitySvmProgramSwap),
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

// Opportunity can be refreshed after 30 seconds
const MIN_REFRESH_TIME: Duration = Duration::seconds(30);

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
            OpportunitySvmProgram::Swap(program) => {
                repository::OpportunityMetadataSvmProgram::Swap(
                    repository::OpportunityMetadataSvmProgramSwap {
                        user_wallet_address:  program.user_wallet_address,
                        fee_token:            program.fee_token,
                        referral_fee_bps:     program.referral_fee_bps,
                        input_token_program:  program.input_token_program,
                        output_token_program: program.output_token_program,
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

    fn get_opportunity_delete(&self) -> api::OpportunityDelete {
        api::OpportunityDelete::Svm(api::OpportunityDeleteSvm::V1(api::OpportunityDeleteV1Svm {
            chain_id:           self.chain_id.clone(),
            permission_account: self.permission_account,
            router:             self.router,
            program:            self.program.clone().into(),
        }))
    }

    fn compare(&self, other: &Self::OpportunityCreate) -> super::OpportunityComparison {
        let mut self_clone: OpportunityCreateSvm = self.clone().into();
        self_clone.slot = other.slot;
        if *other == self_clone {
            if self.refresh_time + MIN_REFRESH_TIME < OffsetDateTime::now_utc() {
                OpportunityComparison::NeedsRefresh
            } else {
                OpportunityComparison::Duplicate
            }
        } else {
            OpportunityComparison::New
        }
    }

    fn refresh(&mut self) {
        self.core_fields.refresh_time = OffsetDateTime::now_utc();
    }
}

impl OpportunityCreate for OpportunityCreateSvm {
    type ApiOpportunityCreate = api::OpportunityCreateSvm;

    fn get_key(&self) -> super::OpportunityKey {
        super::OpportunityKey(
            self.core_fields.chain_id.clone(),
            self.core_fields.permission_key.clone(),
        )
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
pub fn get_swap_quote_tokens(opp: &OpportunitySvm) -> QuoteTokens {
    if !matches!(opp.program, OpportunitySvmProgram::Swap(_)) {
        panic!("Opportunity must be a swap opportunity to get quote tokens");
    }
    let opp_sell_token = opp
        .core_fields
        .sell_tokens
        .first()
        .expect("Swap opportunity sell tokens must not be empty");
    let opp_buy_token = opp
        .core_fields
        .buy_tokens
        .first()
        .expect("Swap opportunity buy tokens must not be empty");
    match (opp_sell_token.amount, opp_buy_token.amount) {
        (_, 0) => QuoteTokens::InputTokenSpecified {
            input_token:  opp_sell_token.clone(),
            output_token: opp_buy_token.token,
        },
        (0, _) => QuoteTokens::OutputTokenSpecified {
            input_token:  opp_sell_token.token,
            output_token: opp_buy_token.clone(),
        },
        _ => {
            panic!("Non zero amount for both sell and buy tokens in swap opportunity");
        }
    }
}
impl From<OpportunitySvm> for api::OpportunitySvm {
    fn from(val: OpportunitySvm) -> Self {
        let program = match val.program.clone() {
            OpportunitySvmProgram::Limo(program) => api::OpportunityParamsV1ProgramSvm::Limo {
                order:         program.order,
                order_address: program.order_address,
            },
            OpportunitySvmProgram::Swap(program) => {
                let quote_tokens = get_swap_quote_tokens(&val);

                let tokens = match quote_tokens {
                    QuoteTokens::InputTokenSpecified {
                        input_token,
                        output_token,
                    } => api::QuoteTokens::InputTokenSpecified {
                        input_token: input_token.into(),
                        output_token,
                        input_token_program: program.input_token_program,
                        output_token_program: program.output_token_program,
                    },
                    QuoteTokens::OutputTokenSpecified {
                        input_token,
                        output_token,
                    } => api::QuoteTokens::OutputTokenSpecified {
                        input_token,
                        output_token: output_token.into(),
                        input_token_program: program.input_token_program,
                        output_token_program: program.output_token_program,
                    },
                };

                let fee_token = match program.fee_token {
                    FeeToken::InputToken => api::FeeToken::InputToken,
                    FeeToken::OutputToken => api::FeeToken::OutputToken,
                };
                api::OpportunityParamsV1ProgramSvm::Swap {
                    user_wallet_address: program.user_wallet_address,
                    permission_account: val.permission_account,
                    router_account: val.router,
                    fee_token,
                    referral_fee_bps: program.referral_fee_bps,
                    // TODO can we make it type safe?
                    tokens,
                }
            }
        };
        api::OpportunitySvm {
            opportunity_id: val.id,
            creation_time:  val.creation_time.unix_timestamp_nanos() / 1000,
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
            repository::OpportunityMetadataSvmProgram::Swap(program) => {
                OpportunitySvmProgram::Swap(OpportunitySvmProgramSwap {
                    user_wallet_address:  program.user_wallet_address,
                    fee_token:            program.fee_token,
                    referral_fee_bps:     program.referral_fee_bps,
                    input_token_program:  program.input_token_program,
                    output_token_program: program.output_token_program,
                })
            }
        };
        Ok(OpportunitySvm {
            core_fields: OpportunityCoreFields {
                id: val.id,
                creation_time: val.creation_time.assume_utc(),
                refresh_time: val.creation_time.assume_utc(),
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
            // TODO*: this arm doesn't matter bc this conversion is only called in `post_opportunity` in api.rs. but we should handle this better
            api::OpportunityCreateProgramParamsV1Svm::Swap {
                user_wallet_address,
                referral_fee_bps,
                input_token_program,
                output_token_program,
            } => OpportunitySvmProgram::Swap(OpportunitySvmProgramSwap {
                user_wallet_address,
                // TODO*: see comment above about this arm
                fee_token: FeeToken::InputToken,
                referral_fee_bps,
                input_token_program,
                output_token_program,
            }),
        };

        let bid_instruction_type = match program {
            OpportunitySvmProgram::Limo(_) => BidPaymentInstructionType::SubmitBid,
            OpportunitySvmProgram::Swap(_) => BidPaymentInstructionType::Swap,
        };

        OpportunityCreateSvm {
            core_fields: OpportunityCoreFieldsCreate::<TokenAmountSvm> {
                permission_key: get_perission_key(
                    bid_instruction_type,
                    params.router,
                    params.permission_account,
                ),
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

fn get_perission_key(
    bid_type: BidPaymentInstructionType,
    router: Pubkey,
    permission_account: Pubkey,
) -> PermissionKey {
    let mut permission_key: [u8; 65] = [0; 65];
    permission_key[0] = bid_type.into();
    permission_key[1..33].copy_from_slice(&router.to_bytes());
    permission_key[33..65].copy_from_slice(&permission_account.to_bytes());
    permission_key.into()
}

impl OpportunitySvm {
    pub fn get_missing_signers(&self) -> Vec<Pubkey> {
        match self.program.clone() {
            OpportunitySvmProgram::Swap(data) => vec![data.user_wallet_address],
            OpportunitySvmProgram::Limo(_) => vec![],
        }
    }

    // TODO It's not good to use another module type here
    pub fn get_permission_key(
        bid_type: BidPaymentInstructionType,
        router: Pubkey,
        permission_account: Pubkey,
    ) -> PermissionKey {
        get_perission_key(bid_type, router, permission_account)
    }
}

impl From<OpportunitySvmProgram> for api::ProgramSvm {
    fn from(val: OpportunitySvmProgram) -> Self {
        match val {
            OpportunitySvmProgram::Limo(_) => api::ProgramSvm::Limo,
            OpportunitySvmProgram::Swap(_) => api::ProgramSvm::Swap,
        }
    }
}
