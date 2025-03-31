use {
    crate::ClientError,
    express_relay::{
        sdk::helpers::{
            create_submit_bid_instruction,
            create_swap_instruction,
            deserialize_metadata,
        },
        state::{
            ExpressRelayMetadata,
            FEE_SPLIT_PRECISION,
            SEED_METADATA,
        },
        FeeToken,
        SwapArgs,
    },
    express_relay_api_types::opportunity::{
        FeeToken as ApiFeeToken,
        OpportunityParamsSvm,
        OpportunityParamsV1ProgramSvm,
        QuoteTokens,
        QuoteTokensWithTokenPrograms,
        TokenAccountInitializationConfig,
        TokenAccountInitializationConfigs,
    },
    solana_rpc_client::nonblocking::rpc_client::RpcClient,
    solana_sdk::{
        clock::Slot,
        hash::Hash,
        instruction::Instruction,
        program_pack::Pack,
        pubkey::Pubkey,
        rent::Rent,
        signature::Keypair,
        system_instruction::transfer,
    },
    spl_associated_token_account::{
        get_associated_token_address,
        get_associated_token_address_with_program_id,
        instruction::create_associated_token_account_idempotent,
    },
    spl_token::{
        instruction::{
            close_account,
            sync_native,
        },
        state::Account as TokenAccount,
    },
    std::str::FromStr,
};

pub struct ProgramParamsLimo {
    pub permission: Pubkey,
    pub router:     Pubkey,
}

pub struct ProgramParamsSwap {}

pub enum ProgramParams {
    Limo(ProgramParamsLimo),
    Swap(ProgramParamsSwap),
}

pub struct NewBidParams {
    pub amount:               u64,
    pub deadline:             i64,
    pub block_hash:           Hash,
    pub slot:                 Option<Slot>,
    pub instructions:         Vec<Instruction>,
    pub payer:                Pubkey,
    pub searcher:             Pubkey,
    pub signers:              Vec<Keypair>,
    pub fee_receiver_relayer: Pubkey,
    pub relayer_signer:       Pubkey,
    pub program_params:       ProgramParams,
}

pub struct GetSubmitBidInstructionParams {
    pub chain_id:             String,
    pub amount:               u64,
    pub deadline:             i64,
    pub searcher:             Pubkey,
    pub permission:           Pubkey,
    pub router:               Pubkey,
    pub relayer_signer:       Pubkey,
    pub fee_receiver_relayer: Pubkey,
}

pub struct GetTokenAccountToCreateParams {
    pub searcher: Pubkey,
    pub user:     Pubkey,
    pub params:   TokenAccountInitializationParams,
}

pub struct GetTokenAccountsToCreateParams {
    pub searcher:               Pubkey,
    pub user:                   Pubkey,
    pub router:                 Pubkey,
    pub fee_receiver_relayer:   Pubkey,
    pub express_relay_metadata: Pubkey,
    pub mint_searcher:          Pubkey,
    pub token_program_searcher: Pubkey,
    pub mint_user:              Pubkey,
    pub token_program_user:     Pubkey,
    pub mint_fee:               Pubkey,
    pub fee_token_program:      Pubkey,
    pub configs:                TokenAccountInitializationConfigs,
}

pub struct TokenAccountToCreate {
    pub payer:   Pubkey,
    pub owner:   Pubkey,
    pub mint:    Pubkey,
    pub program: Pubkey,
}

pub struct TokenAccountInitializationParams {
    pub owner:   Pubkey,
    pub mint:    Pubkey,
    pub program: Pubkey,
    pub config:  TokenAccountInitializationConfig,
}

pub struct GetSwapInstructionParams {
    pub searcher:             Pubkey,
    pub opportunity_params:   OpportunityParamsSvm,
    pub bid_amount:           u64,
    pub deadline:             i64,
    pub fee_receiver_relayer: Pubkey,
    pub relayer_signer:       Pubkey,
}

struct OpportunitySwapData<'a> {
    user:             &'a Pubkey,
    tokens:           &'a QuoteTokensWithTokenPrograms,
    fee_token:        &'a ApiFeeToken,
    router_account:   &'a Pubkey,
    referral_fee_bps: &'a u16,
    platform_fee_bps: &'a u64,
}
pub struct GetSwapCreateAccountsIdempotentInstructionsParams {
    pub searcher:               Pubkey,
    pub user:                   Pubkey,
    pub searcher_token:         Pubkey,
    pub token_program_searcher: Pubkey,
    pub mint_user:              Pubkey,
    pub token_program_user:     Pubkey,
    pub fee_token:              Pubkey,
    pub fee_token_program:      Pubkey,
    pub router_account:         Pubkey,
    pub fee_receiver_relayer:   Pubkey,
    pub referral_fee_bps:       u16,
    pub chain_id:               String,
    pub configs:                TokenAccountInitializationConfigs,
}

pub struct GetWrapSolInstructionsParams {
    pub payer:      Pubkey,
    pub owner:      Pubkey,
    pub amount:     u64,
    pub create_ata: bool,
}

pub struct GetUnwrapSolInstructionParams {
    pub owner: Pubkey,
}

pub struct Svm {
    client: RpcClient,
}

impl Svm {
    pub fn new(rpc_url: String) -> Self {
        Self {
            client: RpcClient::new(rpc_url),
        }
    }

    pub async fn get_express_relay_metadata(
        &self,
        chain_id: String,
    ) -> Result<ExpressRelayMetadata, ClientError> {
        let express_relay_metadata =
            Pubkey::find_program_address(&[SEED_METADATA], &Self::get_express_relay_pid(chain_id))
                .0;

        let data = self
            .client
            .get_account_data(&express_relay_metadata)
            .await
            .map_err(|_| {
                ClientError::SvmError("Failed to fetch express relay metadata".to_string())
            })?;

        match deserialize_metadata(data) {
            Ok(metadata) => Ok(metadata),
            Err(e) => Err(ClientError::SvmError(format!(
                "Failed to deserialize express relay metadata: {:?}",
                e
            ))),
        }
    }

    pub fn get_express_relay_pid(chain_id: String) -> Pubkey {
        if chain_id == "development-solana" {
            Pubkey::from_str("stag1NN9voD7436oFvKmy1kvRZYLLW8drKocSCt2W79")
                .expect("Failed to parse express relay pubkey")
        } else {
            express_relay::id()
        }
    }

    pub fn get_submit_bid_instruction(
        params: GetSubmitBidInstructionParams,
    ) -> Result<Instruction, ClientError> {
        Ok(create_submit_bid_instruction(
            Self::get_express_relay_pid(params.chain_id),
            params.searcher,
            params.relayer_signer,
            params.fee_receiver_relayer,
            params.permission,
            params.router,
            params.deadline,
            params.amount,
        ))
    }

    fn get_token_account_to_create(
        params: GetTokenAccountToCreateParams,
    ) -> Option<TokenAccountToCreate> {
        let GetTokenAccountToCreateParams {
            searcher,
            user,
            params,
        } = params;
        let TokenAccountInitializationParams {
            config,
            owner,
            mint,
            program,
        } = params;

        if config == TokenAccountInitializationConfig::Unneeded {
            return None;
        }
        Some(TokenAccountToCreate {
            payer: if config == TokenAccountInitializationConfig::SearcherPayer {
                searcher
            } else {
                user
            },
            owner,
            mint,
            program,
        })
    }

    fn get_token_accounts_to_create(
        params: GetTokenAccountsToCreateParams,
    ) -> Vec<TokenAccountToCreate> {
        let token_accounts_initialization_params = [
            TokenAccountInitializationParams {
                config:  params.configs.user_ata_mint_searcher,
                owner:   params.user,
                mint:    params.mint_searcher,
                program: params.token_program_searcher,
            },
            TokenAccountInitializationParams {
                config:  params.configs.user_ata_mint_user,
                owner:   params.user,
                mint:    params.mint_user,
                program: params.token_program_user,
            },
            TokenAccountInitializationParams {
                config:  params.configs.router_fee_receiver_ta,
                owner:   params.router,
                mint:    params.mint_fee,
                program: params.fee_token_program,
            },
            TokenAccountInitializationParams {
                config:  params.configs.relayer_fee_receiver_ata,
                owner:   params.fee_receiver_relayer,
                mint:    params.mint_fee,
                program: params.fee_token_program,
            },
            TokenAccountInitializationParams {
                config:  params.configs.express_relay_fee_receiver_ata,
                owner:   params.express_relay_metadata,
                mint:    params.mint_fee,
                program: params.fee_token_program,
            },
        ];
        token_accounts_initialization_params
            .into_iter()
            .filter_map(|token_account_initialization_params| {
                Self::get_token_account_to_create(GetTokenAccountToCreateParams {
                    searcher: params.searcher,
                    user:     params.user,
                    params:   token_account_initialization_params,
                })
            })
            .collect()
    }

    fn extract_swap_data(
        opportunity_params: &OpportunityParamsSvm,
    ) -> Result<OpportunitySwapData, ClientError> {
        let OpportunityParamsSvm::V1(opportunity_params) = opportunity_params;
        match &opportunity_params.program {
            OpportunityParamsV1ProgramSvm::Swap {
                user_wallet_address,
                tokens,
                fee_token,
                referral_fee_bps,
                router_account,
                platform_fee_bps,
                ..
            } => Ok(OpportunitySwapData {
                user: user_wallet_address,
                tokens,
                fee_token,
                router_account,
                referral_fee_bps,
                platform_fee_bps,
            }),
            _ => Err(ClientError::SvmError(
                "Invalid opportunity program".to_string(),
            )),
        }
    }

    pub fn get_memo_instruction(memo: String) -> Instruction {
        spl_memo_client::instructions::AddMemoBuilder::new()
            .memo(memo.into())
            .instruction()
    }

    pub fn get_swap_create_accounts_idempotent_instructions(
        params: GetSwapCreateAccountsIdempotentInstructionsParams,
    ) -> Vec<Instruction> {
        let express_relay_metadata = Pubkey::find_program_address(
            &[SEED_METADATA],
            &Self::get_express_relay_pid(params.chain_id),
        )
        .0;
        let token_accounts_to_create =
            Self::get_token_accounts_to_create(GetTokenAccountsToCreateParams {
                searcher: params.searcher,
                user: params.user,
                router: params.router_account,
                fee_receiver_relayer: params.fee_receiver_relayer,
                express_relay_metadata,
                mint_searcher: params.searcher_token,
                token_program_searcher: params.token_program_searcher,
                mint_user: params.mint_user,
                token_program_user: params.token_program_user,
                mint_fee: params.fee_token,
                fee_token_program: params.fee_token_program,
                configs: params.configs,
            });
        token_accounts_to_create
            .into_iter()
            .map(|token_account_to_create| {
                create_associated_token_account_idempotent(
                    &token_account_to_create.payer,
                    &token_account_to_create.owner,
                    &token_account_to_create.mint,
                    &token_account_to_create.program,
                )
            })
            .collect()
    }

    pub fn get_swap_instruction(
        params: GetSwapInstructionParams,
    ) -> Result<Instruction, ClientError> {
        let swap_data = Self::extract_swap_data(&params.opportunity_params)?;

        let OpportunityParamsSvm::V1(opportunity_params) = &params.opportunity_params;
        let chain_id = opportunity_params.chain_id.clone();

        let bid_amount =
            Self::get_bid_amount_including_fees(&params.opportunity_params, params.bid_amount)?;

        let token_program_searcher = swap_data.tokens.token_program_searcher;
        let token_program_user = swap_data.tokens.token_program_user;
        let (mint_searcher, mint_user, amount_searcher, amount_user) = match swap_data.tokens.tokens
        {
            QuoteTokens::SearcherTokenSpecified {
                searcher_token,
                user_token,
                searcher_amount,
            } => (searcher_token, user_token, searcher_amount, bid_amount),
            QuoteTokens::UserTokenSpecified {
                searcher_token,
                user_token,
                user_amount: _user_amount, // Only for searcher internal pricing
                user_amount_including_fees,
            } => (
                searcher_token,
                user_token,
                bid_amount,
                user_amount_including_fees,
            ),
        };

        let (fee_token, fee_token_mint, fee_token_program) = match swap_data.fee_token {
            ApiFeeToken::SearcherToken => {
                (FeeToken::Searcher, mint_searcher, token_program_searcher)
            }
            ApiFeeToken::UserToken => (FeeToken::User, mint_user, token_program_user),
        };

        let router_fee_receiver_ta = get_associated_token_address_with_program_id(
            swap_data.router_account,
            &fee_token_mint,
            &fee_token_program,
        );

        let swap_args = SwapArgs {
            deadline: params.deadline,
            amount_searcher,
            amount_user,
            referral_fee_bps: *swap_data.referral_fee_bps,
            fee_token,
        };

        Ok(create_swap_instruction(
            Self::get_express_relay_pid(chain_id),
            params.searcher,
            *swap_data.user,
            None,
            None,
            router_fee_receiver_ta,
            params.fee_receiver_relayer,
            mint_searcher,
            mint_user,
            token_program_searcher,
            token_program_user,
            swap_args,
            params.relayer_signer,
        ))
    }

    pub fn get_wrap_sol_instructions(
        params: GetWrapSolInstructionsParams,
    ) -> Result<Vec<Instruction>, ClientError> {
        let mut instructions = vec![];
        if params.create_ata {
            instructions.push(create_associated_token_account_idempotent(
                &params.payer,
                &params.owner,
                &spl_token::native_mint::id(),
                &spl_token::id(),
            ));
        };
        let ata = get_associated_token_address(&params.owner, &spl_token::native_mint::id());
        instructions.push(transfer(&params.owner, &ata, params.amount));
        instructions.push(sync_native(&spl_token::id(), &ata).map_err(|e| {
            ClientError::SvmError(format!("Failed to create sync native instruction: {:?}", e))
        })?);
        Ok(instructions)
    }

    pub fn get_unwrap_sol_instruction(
        params: GetUnwrapSolInstructionParams,
    ) -> Result<Instruction, ClientError> {
        let ata = get_associated_token_address(&params.owner, &spl_token::native_mint::id());
        close_account(&spl_token::id(), &ata, &params.owner, &params.owner, &[]).map_err(|e| {
            ClientError::SvmError(format!(
                "Failed to create close account instruction: {:?}",
                e
            ))
        })
    }

    pub fn get_user_amount_to_wrap(
        amount_user: u64,
        user_mint_user_balance: u64,
        token_account_initialization_configs: &TokenAccountInitializationConfigs,
    ) -> u64 {
        let number_of_paid_atas_by_user = [
            &token_account_initialization_configs.user_ata_mint_user,
            &token_account_initialization_configs.user_ata_mint_searcher,
        ]
        .iter()
        .filter(|&&config| matches!(config, TokenAccountInitializationConfig::UserPayer))
        .count();

        std::cmp::min(
            amount_user,
            user_mint_user_balance.saturating_sub(
                number_of_paid_atas_by_user as u64
                    * Rent::default().minimum_balance(TokenAccount::LEN),
            ),
        )
    }

    /// Adjusts the bid amount in the case where the amount that needs to be provided by the searcher is specified and the fees are in the user token.
    /// In this case, searchers' bids represent how many tokens they would like to receive.
    /// However, for the searcher to receive `bidAmount`, the user needs to provide `bidAmount * (FEE_SPLIT_PRECISION / (FEE_SPLIT_PRECISION - fees))`
    /// This function handles this adjustment.
    pub fn get_bid_amount_including_fees(
        opportunity: &OpportunityParamsSvm,
        bid_amount: u64,
    ) -> Result<u64, ClientError> {
        let swap_data = Self::extract_swap_data(opportunity)?;
        Ok(match (&swap_data.tokens.tokens, &swap_data.fee_token) {
            // scale bid amount by FEE_SPLIT_PRECISION/(FEE_SPLIT_PRECISION-fees) to account for fees
            (QuoteTokens::SearcherTokenSpecified { .. }, ApiFeeToken::UserToken) => {
                let denominator = FEE_SPLIT_PRECISION
                    - <u16 as Into<u64>>::into(*swap_data.referral_fee_bps)
                    - *swap_data.platform_fee_bps;
                let numerator = bid_amount * FEE_SPLIT_PRECISION;
                numerator.div_ceil(denominator)
            }
            _ => bid_amount,
        })
    }
}
