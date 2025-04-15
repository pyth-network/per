use {
    crate::{
        api::RestError,
        auction::{
            entities::BidPaymentInstructionType,
            service::verification::{
                get_current_time_rounded_with_offset,
                BID_MINIMUM_LIFE_TIME_SVM_OTHER,
            },
        },
        kernel::entities::{
            ChainId,
            PermissionKey,
        },
        opportunity::{
            entities::{
                QuoteTokens,
                TokenAmountSvm,
            },
            repository,
        },
    },
    ::express_relay::FeeToken as ProgramFeeToken,
    ethers::types::Bytes,
    express_relay::state::FEE_SPLIT_PRECISION,
    express_relay_api_types::opportunity::{
        self as api,
        QuoteTokensWithTokenPrograms,
    },
    serde::{
        Deserialize,
        Serialize,
    },
    solana_sdk::{
        clock::Slot,
        program_pack::Pack,
        pubkey::Pubkey,
        rent::Rent,
    },
    spl_token_2022::state::Account as TokenAccount,
    std::{
        fmt::Debug,
        time::Duration,
    },
    time::OffsetDateTime,
    uuid::Uuid,
};

pub type OpportunityId = Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OpportunityKey(pub ChainId, pub PermissionKey);

#[derive(Debug, Clone)]
pub enum OpportunityComparison {
    New,
    Duplicate,
    NeedsRefresh,
}

#[derive(Debug)]
pub enum OpportunityRemovalReason {
    Expired,
    // TODO use internal errors instead of RestError
    #[allow(dead_code)]
    Invalid(RestError),
}

impl From<OpportunityRemovalReason> for repository::OpportunityRemovalReason {
    fn from(reason: OpportunityRemovalReason) -> Self {
        match reason {
            OpportunityRemovalReason::Expired => repository::OpportunityRemovalReason::Expired,
            OpportunityRemovalReason::Invalid(_) => repository::OpportunityRemovalReason::Invalid,
        }
    }
}

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

    pub fn searcher_payer() -> Self {
        Self {
            user_ata_mint_searcher:         TokenAccountInitializationConfig::SearcherPayer,
            user_ata_mint_user:             TokenAccountInitializationConfig::Unneeded,
            router_fee_receiver_ta:         TokenAccountInitializationConfig::SearcherPayer,
            relayer_fee_receiver_ata:       TokenAccountInitializationConfig::SearcherPayer,
            express_relay_fee_receiver_ata: TokenAccountInitializationConfig::SearcherPayer,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct OpportunitySvmProgramSwap {
    pub user_wallet_address:                  Pubkey,
    pub user_mint_user_balance:               u64,
    pub fee_token:                            FeeToken,
    pub referral_fee_bps:                     u16,
    pub platform_fee_bps:                     u64,
    // TODO*: these really should not live here. they should live in the opportunity core fields, but we don't want to introduce a breaking change. in any case, the need for the token programs is another sign that quotes should be separated from the traditional opportunity struct.
    pub token_program_user:                   Pubkey,
    pub token_program_searcher:               Pubkey,
    pub token_account_initialization_configs: TokenAccountInitializationConfigs,
    pub memo:                                 Option<String>,
    pub cancellable:                          bool,
    pub minimum_lifetime:                     Option<u32>,
    pub minimum_deadline:                     OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq)]
pub enum OpportunitySvmProgram {
    Limo(OpportunitySvmProgramLimo),
    Swap(OpportunitySvmProgramSwap),
}

#[derive(Debug, Clone, PartialEq)]
pub struct OpportunitySvm {
    pub id:                 OpportunityId,
    pub permission_key:     Bytes,
    pub chain_id:           ChainId,
    pub sell_tokens:        Vec<TokenAmountSvm>,
    pub buy_tokens:         Vec<TokenAmountSvm>,
    pub creation_time:      OffsetDateTime,
    pub refresh_time:       OffsetDateTime,
    pub router:             Pubkey,
    pub permission_account: Pubkey,
    pub program:            OpportunitySvmProgram,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OpportunityCreateSvm {
    pub permission_key:     Bytes,
    pub chain_id:           ChainId,
    pub sell_tokens:        Vec<TokenAmountSvm>,
    pub buy_tokens:         Vec<TokenAmountSvm>,
    pub router:             Pubkey,
    pub permission_account: Pubkey,
    pub program:            OpportunitySvmProgram,
}

impl OpportunityCreateSvm {
    pub fn get_key(&self) -> OpportunityKey {
        OpportunityKey(self.chain_id.clone(), self.permission_key.clone())
    }
}

// Opportunity can be refreshed after 30 seconds
const MIN_REFRESH_TIME: Duration = Duration::from_secs(30);

impl OpportunitySvm {
    pub fn get_key(&self) -> OpportunityKey {
        OpportunityKey(self.chain_id.clone(), self.permission_key.clone())
    }

    pub fn new_with_current_time(val: OpportunityCreateSvm) -> Self {
        Self {
            id:                 Uuid::new_v4(),
            permission_key:     val.permission_key,
            chain_id:           val.chain_id,
            sell_tokens:        val.sell_tokens,
            buy_tokens:         val.buy_tokens,
            creation_time:      OffsetDateTime::now_utc(),
            refresh_time:       OffsetDateTime::now_utc(),
            router:             val.router,
            permission_account: val.permission_account,
            program:            val.program,
        }
    }

    pub fn get_models_metadata(&self) -> repository::OpportunityMetadataSvm {
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
                        user_wallet_address:                  program.user_wallet_address,
                        fee_token:                            program.fee_token,
                        referral_fee_bps:                     program.referral_fee_bps,
                        platform_fee_bps:                     program.platform_fee_bps,
                        token_program_user:                   program.token_program_user,
                        token_program_searcher:               program.token_program_searcher,
                        token_account_initialization_configs: program
                            .token_account_initialization_configs,
                        user_mint_user_balance:               program.user_mint_user_balance,
                        memo:                                 program.memo,
                        cancellable:                          program.cancellable,
                        minimum_lifetime:                     program.minimum_lifetime,
                    },
                )
            }
        };
        repository::OpportunityMetadataSvm {
            program,
            router: self.router,
            permission_account: self.permission_account,
        }
    }

    pub fn get_opportunity_delete(&self) -> api::OpportunityDelete {
        api::OpportunityDelete::Svm(api::OpportunityDeleteSvm::V1(api::OpportunityDeleteV1Svm {
            chain_id:           self.chain_id.clone(),
            permission_account: self.permission_account,
            router:             self.router,
            program:            self.program.clone().into(),
        }))
    }

    pub fn compare(&self, other: &OpportunityCreateSvm) -> super::OpportunityComparison {
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

    pub fn refresh(&mut self) {
        self.refresh_time = OffsetDateTime::now_utc();
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
        .sell_tokens
        .first()
        .expect("Swap opportunity sell tokens must not be empty");
    let opp_buy_token = opp
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

pub fn get_opportunity_swap_data(opp: &OpportunitySvm) -> &OpportunitySvmProgramSwap {
    match &opp.program {
        OpportunitySvmProgram::Swap(opportunity_swap_data) => opportunity_swap_data,
        _ => {
            panic!("Opportunity must be a swap opportunity to get swap data");
        }
    }
}

impl OpportunitySvmProgramSwap {
    pub fn get_user_amount_to_wrap(&self, amount_user: u64) -> u64 {
        let number_of_atas_paid_by_user = [
            &self.token_account_initialization_configs.user_ata_mint_user,
            &self
                .token_account_initialization_configs
                .user_ata_mint_searcher,
        ]
        .iter()
        .filter(|&&config| matches!(config, TokenAccountInitializationConfig::UserPayer))
        .count();

        std::cmp::min(
            amount_user,
            self.user_mint_user_balance.saturating_sub(
                number_of_atas_paid_by_user as u64
                    * Rent::default().minimum_balance(TokenAccount::LEN), // todo: token2022 accounts can be bigger than this, this hack might not work for them
            ),
        )
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
                                let router_fee = (u128::from(user_token.amount)
                                    * u128::from(program.referral_fee_bps) // this multiplication is safe because user_token.amount and program.referral_fee_bps are u64
                                    / u128::from(FEE_SPLIT_PRECISION))
                                    as u64; // this cast is safe because we know referral_fee_bps is less than FEE_SPLIT_PRECISION
                                let platform_fee = (u128::from(user_token.amount)
                                    * u128::from(program.platform_fee_bps) // this multiplication is safe because user_token.amount and program.platform_fee_bps are u64
                                    / u128::from(FEE_SPLIT_PRECISION))
                                    as u64; // this cast is safe because we know platform_fee_bps is less than FEE_SPLIT_PRECISION
                                user_token
                                    .amount
                                    .saturating_sub(router_fee)
                                    .saturating_sub(platform_fee)
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
                        .token_account_initialization_configs
                        .into(),
                    memo: program.memo,
                    cancellable: program.cancellable,
                    minimum_deadline: program.minimum_deadline.unix_timestamp(),
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
                    user_wallet_address:                  program.user_wallet_address,
                    fee_token:                            program.fee_token,
                    referral_fee_bps:                     program.referral_fee_bps,
                    platform_fee_bps:                     program.platform_fee_bps,
                    token_program_user:                   program.token_program_user,
                    token_program_searcher:               program.token_program_searcher,
                    token_account_initialization_configs: program
                        .token_account_initialization_configs,
                    user_mint_user_balance:               program.user_mint_user_balance,
                    memo:                                 program.memo,
                    cancellable:                          program.cancellable,
                    minimum_lifetime:                     program.minimum_lifetime,
                    minimum_deadline:                     get_current_time_rounded_with_offset(
                        program
                            .minimum_lifetime
                            .map(|lifetime| Duration::from_secs(lifetime as u64))
                            .unwrap_or(BID_MINIMUM_LIFE_TIME_SVM_OTHER),
                    ),
                })
            }
        };
        Ok(OpportunitySvm {
            id: val.id,
            creation_time: val.creation_time.assume_utc(),
            refresh_time: val.creation_time.assume_utc(),
            permission_key: PermissionKey::from(val.permission_key),
            chain_id: val.chain_id,
            sell_tokens,
            buy_tokens,
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
            permission_key: get_permission_key(
                bid_instruction_type,
                params.router,
                params.permission_account,
            ),
            chain_id: params.chain_id,
            sell_tokens: params.sell_tokens.into_iter().map(|t| t.into()).collect(),
            buy_tokens: params.buy_tokens.into_iter().map(|t| t.into()).collect(),
            program,
            permission_account: params.permission_account,
            router: params.router,
        }
    }
}

impl From<OpportunitySvm> for OpportunityCreateSvm {
    fn from(val: OpportunitySvm) -> Self {
        OpportunityCreateSvm {
            permission_key:     val.permission_key,
            chain_id:           val.chain_id,
            sell_tokens:        val.sell_tokens,
            buy_tokens:         val.buy_tokens,
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
        signers: &[Pubkey],
        relayer_signer: &Pubkey,
    ) -> Result<(), anyhow::Error> {
        let fee_payer = signers
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
