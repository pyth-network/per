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
    express_relay::state::FEE_SPLIT_PRECISION,
    express_relay_api_types::{
        opportunity as api,
        opportunity::QuoteTokensWithTokenPrograms,
    },
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
    pub slot:          Slot,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeeToken {
    UserToken,
    SearcherToken,
}

impl PartialEq<ProgramFeeToken> for FeeToken {
    fn eq(&self, other: &ProgramFeeToken) -> bool {
        match self {
            FeeToken::SearcherToken => matches!(other, ProgramFeeToken::Searcher),
            FeeToken::UserToken => matches!(other, ProgramFeeToken::User),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TokenAccountInitializationConfig {
    Unneeded,
    SearcherPayer,
    UserPayer,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TokenAccountInitializationConfigs {
    pub user_ata_mint_searcher:         TokenAccountInitializationConfig,
    pub user_ata_mint_user:             TokenAccountInitializationConfig,
    pub router_fee_receiver_ta:         TokenAccountInitializationConfig,
    pub relayer_fee_receiver_ata:       TokenAccountInitializationConfig,
    pub express_relay_fee_receiver_ata: TokenAccountInitializationConfig,
}

impl TokenAccountInitializationConfigs {
    pub fn none_needed() -> Self {
        Self {
            user_ata_mint_searcher:         TokenAccountInitializationConfig::Unneeded,
            user_ata_mint_user:             TokenAccountInitializationConfig::Unneeded,
            router_fee_receiver_ta:         TokenAccountInitializationConfig::Unneeded,
            relayer_fee_receiver_ata:       TokenAccountInitializationConfig::Unneeded,
            express_relay_fee_receiver_ata: TokenAccountInitializationConfig::Unneeded,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct OpportunitySvmProgramSwap {
    pub user_wallet_address:                 Pubkey,
    pub user_mint_user_balance:              u64,
    pub fee_token:                           FeeToken,
    pub referral_fee_bps:                    u16,
    pub platform_fee_bps:                    u64,
    // TODO*: these really should not live here. they should live in the opportunity core fields, but we don't want to introduce a breaking change. in any case, the need for the token programs is another sign that quotes should be separated from the traditional opportunity struct.
    pub token_program_user:                  Pubkey,
    pub token_program_searcher:              Pubkey,
    pub token_account_initialization_config: TokenAccountInitializationConfigs,
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
}

#[derive(Debug, Clone, PartialEq)]
pub struct OpportunityCreateSvm {
    pub core_fields: OpportunityCoreFieldsCreate<TokenAmountSvm>,

    pub router:             Pubkey,
    pub permission_account: Pubkey,
    pub program:            OpportunitySvmProgram,
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
        }
    }

    fn get_models_metadata(&self) -> Self::ModelMetadata {
        let program = match self.program.clone() {
            OpportunitySvmProgram::Limo(program) => {
                repository::OpportunityMetadataSvmProgram::Limo(
                    repository::OpportunityMetadataSvmProgramLimo {
                        order:         program.order,
                        order_address: program.order_address,
                        slot:          program.slot,
                    },
                )
            }
            OpportunitySvmProgram::Swap(program) => {
                repository::OpportunityMetadataSvmProgram::Swap(
                    repository::OpportunityMetadataSvmProgramSwap {
                        user_wallet_address:                 program.user_wallet_address,
                        fee_token:                           program.fee_token,
                        referral_fee_bps:                    program.referral_fee_bps,
                        platform_fee_bps:                    program.platform_fee_bps,
                        token_program_user:                  program.token_program_user,
                        token_program_searcher:              program.token_program_searcher,
                        token_account_initialization_config: program
                            .token_account_initialization_config,
                        user_mint_user_balance:              program.user_mint_user_balance,
                    },
                )
            }
        };
        Self::ModelMetadata {
            program,
            router: self.router,
            permission_account: self.permission_account,
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

    fn compare(&self, other: &OpportunityCreateSvm) -> super::OpportunityComparison {
        let mut self_clone: OpportunityCreateSvm = self.clone().into();
        if let (
            OpportunitySvmProgram::Limo(self_program),
            OpportunitySvmProgram::Limo(other_program),
        ) = (&mut self_clone.program, &other.program)
        {
            self_program.slot = other_program.slot;
        };
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
    match (opp_buy_token.amount, opp_sell_token.amount) {
        (_, 0) => QuoteTokens::UserTokenSpecified {
            user_token:     opp_buy_token.clone(),
            searcher_token: opp_sell_token.token,
        },
        (0, _) => QuoteTokens::SearcherTokenSpecified {
            user_token:     opp_buy_token.token,
            searcher_token: opp_sell_token.clone(),
        },
        _ => {
            panic!("Non zero amount for both sell and buy tokens in swap opportunity");
        }
    }
}

impl From<TokenAccountInitializationConfig> for api::TokenAccountInitializationConfig {
    fn from(val: TokenAccountInitializationConfig) -> Self {
        match val {
            TokenAccountInitializationConfig::Unneeded => {
                api::TokenAccountInitializationConfig::Unneeded
            }
            TokenAccountInitializationConfig::SearcherPayer => {
                api::TokenAccountInitializationConfig::SearcherPayer
            }
            TokenAccountInitializationConfig::UserPayer => {
                api::TokenAccountInitializationConfig::UserPayer
            }
        }
    }
}

impl From<TokenAccountInitializationConfigs> for api::TokenAccountInitializationConfigs {
    fn from(val: TokenAccountInitializationConfigs) -> Self {
        api::TokenAccountInitializationConfigs {
            user_ata_mint_searcher:         val.user_ata_mint_searcher.into(),
            user_ata_mint_user:             val.user_ata_mint_user.into(),
            router_fee_receiver_ta:         val.router_fee_receiver_ta.into(),
            relayer_fee_receiver_ata:       val.relayer_fee_receiver_ata.into(),
            express_relay_fee_receiver_ata: val.express_relay_fee_receiver_ata.into(),
        }
    }
}

impl From<OpportunitySvm> for api::OpportunitySvm {
    fn from(val: OpportunitySvm) -> Self {
        let program = match val.program.clone() {
            OpportunitySvmProgram::Limo(program) => api::OpportunityParamsV1ProgramSvm::Limo {
                slot:          program.slot,
                order:         program.order,
                order_address: program.order_address,
            },
            OpportunitySvmProgram::Swap(program) => {
                let quote_tokens = get_swap_quote_tokens(&val);

                let tokens = match quote_tokens {
                    QuoteTokens::SearcherTokenSpecified {
                        user_token,
                        searcher_token,
                    } => api::QuoteTokens::SearcherTokenSpecified {
                        searcher_token: searcher_token.token,
                        searcher_amount: searcher_token.amount,
                        user_token,
                    },
                    QuoteTokens::UserTokenSpecified {
                        user_token,
                        searcher_token,
                    } => {
                        let user_amount_excluding_fees = match program.fee_token {
                            FeeToken::UserToken => {
                                // TODO: Do this calculation based on express relay metadata
                                let router_fee = user_token.amount
                                    * program.referral_fee_bps as u64
                                    / FEE_SPLIT_PRECISION;
                                let platform_fee = user_token.amount * program.platform_fee_bps
                                    / FEE_SPLIT_PRECISION;
                                user_token.amount - router_fee - platform_fee
                            }
                            FeeToken::SearcherToken => user_token.amount,
                        };
                        api::QuoteTokens::UserTokenSpecified {
                            searcher_token,
                            user_token: user_token.token,
                            user_amount: user_amount_excluding_fees,
                            user_amount_including_fees: user_token.amount,
                        }
                    }
                };

                let fee_token = match program.fee_token {
                    FeeToken::UserToken => api::FeeToken::UserToken,
                    FeeToken::SearcherToken => api::FeeToken::SearcherToken,
                };
                api::OpportunityParamsV1ProgramSvm::Swap {
                    user_wallet_address: program.user_wallet_address,
                    user_mint_user_balance: program.user_mint_user_balance,
                    permission_account: val.permission_account,
                    router_account: val.router,
                    fee_token,
                    referral_fee_bps: program.referral_fee_bps,
                    platform_fee_bps: program.platform_fee_bps,
                    tokens: QuoteTokensWithTokenPrograms {
                        tokens,
                        token_program_user: program.token_program_user,
                        token_program_searcher: program.token_program_searcher,
                    },
                    token_account_initialization_configs: program
                        .token_account_initialization_config
                        .into(),
                }
            }
        };
        api::OpportunitySvm {
            opportunity_id: val.id,
            creation_time:  val.creation_time.unix_timestamp_nanos() / 1000,
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
                    slot:          program.slot,
                    order:         program.order,
                    order_address: program.order_address,
                })
            }
            repository::OpportunityMetadataSvmProgram::Swap(program) => {
                OpportunitySvmProgram::Swap(OpportunitySvmProgramSwap {
                    user_wallet_address:                 program.user_wallet_address,
                    fee_token:                           program.fee_token,
                    referral_fee_bps:                    program.referral_fee_bps,
                    platform_fee_bps:                    program.platform_fee_bps,
                    token_program_user:                  program.token_program_user,
                    token_program_searcher:              program.token_program_searcher,
                    token_account_initialization_config: program
                        .token_account_initialization_config,
                    user_mint_user_balance:              program.user_mint_user_balance,
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
                slot: params.slot,
            }),
        };

        let bid_instruction_type = match program {
            OpportunitySvmProgram::Limo(_) => BidPaymentInstructionType::SubmitBid,
            OpportunitySvmProgram::Swap(_) => BidPaymentInstructionType::Swap,
        };

        OpportunityCreateSvm {
            core_fields: OpportunityCoreFieldsCreate::<TokenAmountSvm> {
                permission_key: get_permission_key(
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
        }
    }
}

fn get_permission_key(
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
    pub fn check_fee_payer(
        &self,
        accounts: &[Pubkey],
        relayer_signer: &Pubkey,
    ) -> Result<(), anyhow::Error> {
        let fee_payer = accounts
            .first()
            .ok_or_else(|| anyhow::anyhow!("Accounts should not be empty"))?;
        match self.program.clone() {
            OpportunitySvmProgram::Swap(data) => {
                if data.user_wallet_address == *fee_payer {
                    return Err(anyhow::anyhow!("Fee payer should not be user"));
                }
                if relayer_signer == fee_payer {
                    return Err(anyhow::anyhow!("Fee payer should not be relayer signer"));
                }
                Ok(())
            }
            OpportunitySvmProgram::Limo(_) => Ok(()),
        }
    }

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
        get_permission_key(bid_type, router, permission_account)
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
