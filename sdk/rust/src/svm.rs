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
            SEED_METADATA,
        },
        FeeToken as ContractFeeToken,
        SwapArgs,
    },
    express_relay_api_types::opportunity::{
        FeeToken as ApiFeeToken,
        OpportunityParamsSvm,
        OpportunityParamsV1ProgramSvm,
        QuoteTokens,
    },
    solana_rpc_client::nonblocking::rpc_client::RpcClient,
    solana_sdk::{
        clock::Slot,
        hash::Hash,
        instruction::{
            AccountMeta,
            Instruction,
        },
        pubkey::Pubkey,
        signature::Keypair,
    },
    spl_associated_token_account::{
        get_associated_token_address_with_program_id,
        instruction::create_associated_token_account_idempotent,
    },
    std::str::FromStr,
};

pub struct ProgramParamsLimo {
    pub permission:     Pubkey,
    pub router:         Pubkey,
    pub relayer_signer: Pubkey,
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
}

struct OpportunitySwapData {
    trader:           Pubkey,
    tokens:           QuoteTokens,
    fee_token:        ApiFeeToken,
    router_account:   Pubkey,
    referral_fee_bps: u16,
}

pub struct GetSwapCreateAccountsIdempotentInstructionsParams {
    pub payer:                Pubkey,
    pub trader:               Pubkey,
    pub output_token:         Pubkey,
    pub output_token_program: Pubkey,
    pub fee_token:            Pubkey,
    pub fee_token_program:    Pubkey,
    pub router_account:       Pubkey,
    pub fee_receiver_relayer: Pubkey,
    pub referral_fee_bps:     u16,
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

    pub async fn get_express_relay_metadata(&self) -> Result<ExpressRelayMetadata, ClientError> {
        let express_relay_metadata =
            Pubkey::find_program_address(&[SEED_METADATA], &express_relay::ID.to_bytes().into()).0;

        let data = self
            .client
            .get_account_data(&express_relay_metadata)
            .await
            .map_err(|_| {
                ClientError::SvmError("Failed to fetch express relay metadata".to_string())
            })?;
        deserialize_metadata(data).map_err(|_| {
            ClientError::SvmError("Failed to deserialize express relay metadata".to_string())
        })
    }

    pub fn get_express_relay_pid(chain_id: String) -> Pubkey {
        if chain_id == "development-solana" {
            Pubkey::from_str("stag1NN9voD7436oFvKmy1kvRZYLLW8drKocSCt2W79")
                .expect("Failed to parse express relay pubkey")
        } else {
            express_relay::ID.to_bytes().into()
        }
    }

    pub fn get_submit_bid_instruction(params: GetSubmitBidInstructionParams) -> Instruction {
        let submid_bid_instruction = create_submit_bid_instruction(
            Self::get_express_relay_pid(params.chain_id)
                .to_bytes()
                .into(),
            params.searcher.to_bytes().into(),
            params.relayer_signer.to_bytes().into(),
            params.fee_receiver_relayer.to_bytes().into(),
            params.permission.to_bytes().into(),
            params.router.to_bytes().into(),
            params.deadline,
            params.amount,
        );
        Instruction {
            program_id: submid_bid_instruction.program_id.to_bytes().into(),
            accounts:   submid_bid_instruction
                .accounts
                .iter()
                .map(|account| AccountMeta {
                    pubkey:      account.pubkey.to_bytes().into(),
                    is_signer:   account.is_signer,
                    is_writable: account.is_writable,
                })
                .collect(),
            data:       submid_bid_instruction.data,
        }
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
                ..
            } => Ok(OpportunitySwapData {
                trader: user_wallet_address,
                tokens,
                fee_token,
                router_account,
                referral_fee_bps,
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
            &params.trader,
            &params.output_token,
            &params.output_token_program,
        ));
        instructions.push(create_associated_token_account_idempotent(
            &params.payer,
            &params.fee_receiver_relayer,
            &params.fee_token,
            &params.fee_token_program,
        ));
        instructions.push(create_associated_token_account_idempotent(
            &params.payer,
            &Pubkey::find_program_address(&[SEED_METADATA], &express_relay::ID.to_bytes().into()).0,
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

        let (
            mint_input,
            mint_output,
            amount_input,
            amount_output,
            input_token_program,
            output_token_program,
        ) = match swap_data.tokens {
            QuoteTokens::InputTokenSpecified {
                input_token,
                output_token,
                input_token_program,
                output_token_program,
            } => (
                input_token.token,
                output_token,
                input_token.amount,
                params.bid_amount,
                input_token_program,
                output_token_program,
            ),
            QuoteTokens::OutputTokenSpecified {
                input_token,
                output_token,
                input_token_program,
                output_token_program,
            } => (
                input_token,
                output_token.token,
                params.bid_amount,
                output_token.amount,
                input_token_program,
                output_token_program,
            ),
        };

        let (fee_token, fee_token_mint, fee_token_program) = match swap_data.fee_token {
            ApiFeeToken::InputToken => (ContractFeeToken::Input, mint_input, input_token_program),
            ApiFeeToken::OutputToken => {
                (ContractFeeToken::Output, mint_output, output_token_program)
            }
        };
        let router_fee_receiver_ta = get_associated_token_address_with_program_id(
            &swap_data.router_account,
            &fee_token_mint,
            &fee_token_program,
        );

        let swap_instruction = create_swap_instruction(
            Self::get_express_relay_pid(chain_id).to_bytes().into(),
            params.searcher.to_bytes().into(),
            swap_data.trader.to_bytes().into(),
            None,
            None,
            router_fee_receiver_ta.to_bytes().into(),
            params.fee_receiver_relayer.to_bytes().into(),
            mint_input.to_bytes().into(),
            mint_output.to_bytes().into(),
            input_token_program.to_bytes().into(),
            output_token_program.to_bytes().into(),
            SwapArgs {
                deadline: params.deadline,
                amount_input,
                amount_output,
                referral_fee_bps: swap_data.referral_fee_bps,
                fee_token,
            },
        );

        Ok(Instruction {
            program_id: swap_instruction.program_id.to_bytes().into(),
            accounts:   swap_instruction
                .accounts
                .iter()
                .map(|account| AccountMeta {
                    pubkey:      account.pubkey.to_bytes().into(),
                    is_signer:   account.is_signer,
                    is_writable: account.is_writable,
                })
                .collect(),
            data:       swap_instruction.data,
        })
    }
}
