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
    },
    solana_rpc_client::nonblocking::rpc_client::RpcClient,
    solana_sdk::{
        clock::Slot,
        hash::Hash,
        instruction::Instruction,
        pubkey::Pubkey,
        signature::Keypair,
        system_instruction::transfer,
    },
    spl_associated_token_account::{
        get_associated_token_address,
        get_associated_token_address_with_program_id,
        instruction::create_associated_token_account_idempotent,
    },
    spl_token::instruction::{
        close_account,
        sync_native,
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

pub struct GetSwapInstructionParams {
    pub searcher:             Pubkey,
    pub opportunity_params:   OpportunityParamsSvm,
    pub bid_amount:           u64,
    pub deadline:             i64,
    pub fee_receiver_relayer: Pubkey,
    pub relayer_signer:       Pubkey,
}

struct OpportunitySwapData {
    user:             Pubkey,
    tokens:           QuoteTokensWithTokenPrograms,
    fee_token:        ApiFeeToken,
    router_account:   Pubkey,
    referral_fee_bps: u16,
    platform_fee_bps: u64,
}

pub struct GetSwapCreateAccountsIdempotentInstructionsParams {
    pub payer:                  Pubkey,
    pub user:                   Pubkey,
    pub searcher_token:         Pubkey,
    pub token_program_searcher: Pubkey,
    pub fee_token:              Pubkey,
    pub fee_token_program:      Pubkey,
    pub router_account:         Pubkey,
    pub fee_receiver_relayer:   Pubkey,
    pub referral_fee_bps:       u16,
    pub chain_id:               String,
}

pub struct GetWrapSolInstructionsParams {
    pub payer:  Pubkey,
    pub owner:  Pubkey,
    pub amount: u64,
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

    fn extract_swap_data(
        opportunity_params: OpportunityParamsSvm,
    ) -> Result<OpportunitySwapData, ClientError> {
        let OpportunityParamsSvm::V1(opportunity_params) = opportunity_params;
        match opportunity_params.program {
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

    pub fn get_swap_create_accounts_idempotent_instructions(
        params: GetSwapCreateAccountsIdempotentInstructionsParams,
    ) -> Vec<Instruction> {
        let mut instructions = vec![];
        instructions.push(create_associated_token_account_idempotent(
            &params.payer,
            &params.user,
            &params.searcher_token,
            &params.token_program_searcher,
        ));
        instructions.push(create_associated_token_account_idempotent(
            &params.payer,
            &params.fee_receiver_relayer,
            &params.fee_token,
            &params.fee_token_program,
        ));
        instructions.push(create_associated_token_account_idempotent(
            &params.payer,
            &Pubkey::find_program_address(
                &[SEED_METADATA],
                &Self::get_express_relay_pid(params.chain_id),
            )
            .0,
            &params.fee_token,
            &params.fee_token_program,
        ));
        if params.referral_fee_bps > 0 {
            instructions.push(create_associated_token_account_idempotent(
                &params.payer,
                &params.router_account,
                &params.fee_token,
                &params.fee_token_program,
            ));
        }
        instructions
    }

    pub fn get_swap_instruction(
        params: GetSwapInstructionParams,
    ) -> Result<Instruction, ClientError> {
        let swap_data = Self::extract_swap_data(params.opportunity_params.clone())?;

        let OpportunityParamsSvm::V1(opportunity_params) = params.opportunity_params;
        let chain_id = opportunity_params.chain_id;

        let bid_amount = match (&swap_data.tokens.tokens, &swap_data.fee_token) {
            // scale bid amount by FEE_SPLIT_PRECISION/(FEE_SPLIT_PRECISION-fees) to account for fees
            (QuoteTokens::SearcherTokenSpecified { .. }, ApiFeeToken::UserToken) => {
                let denominator = FEE_SPLIT_PRECISION
                    - <u16 as Into<u64>>::into(swap_data.referral_fee_bps)
                    - swap_data.platform_fee_bps;
                let numerator = params.bid_amount * FEE_SPLIT_PRECISION;
                numerator.div_ceil(denominator)
            }
            _ => params.bid_amount,
        };

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
            &swap_data.router_account,
            &fee_token_mint,
            &fee_token_program,
        );

        let swap_args = SwapArgs {
            deadline: params.deadline,
            amount_searcher,
            amount_user,
            referral_fee_bps: swap_data.referral_fee_bps,
            fee_token,
        };

        Ok(create_swap_instruction(
            Self::get_express_relay_pid(chain_id),
            params.searcher,
            swap_data.user,
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
        instructions.push(create_associated_token_account_idempotent(
            &params.payer,
            &params.owner,
            &spl_token::native_mint::id(),
            &spl_token::id(),
        ));
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
}
