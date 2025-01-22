use {
    crate::ClientError,
    borsh::{
        BorshDeserialize,
        BorshSerialize,
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
        system_program,
        sysvar,
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
    pub chain_id:             String,
}

pub struct Svm {
    client: RpcClient,
}

// After anchor releas version 0.31, we can remove all of the following functions
// And use the ones from the express-relay contract crate
pub const SEED_METADATA: &[u8] = b"metadata";
pub const SEED_CONFIG_ROUTER: &[u8] = b"config_router";

#[derive(BorshDeserialize, BorshSerialize, Debug)]
pub struct ExpressRelayMetadata {
    pub admin:                 Pubkey,
    pub relayer_signer:        Pubkey,
    pub fee_receiver_relayer:  Pubkey,
    // the portion of the bid that goes to the router, in bps
    pub split_router_default:  u64,
    // the portion of the remaining bid (after router fees) that goes to the relayer, in bps
    pub split_relayer:         u64,
    // the portion of the swap amount that should go to the platform (relayer + express relay), in bps
    pub swap_platform_fee_bps: u64,
}

#[derive(BorshSerialize, BorshDeserialize)]
enum FeeToken {
    Input,
    Output,
}

#[derive(BorshSerialize, BorshDeserialize)]
struct SwapArgs {
    pub deadline:         i64,
    pub amount_input:     u64,
    pub amount_output:    u64,
    pub referral_fee_bps: u16,
    pub fee_token:        FeeToken,
}

#[derive(BorshSerialize, BorshDeserialize)]
struct SubmitBidArgs {
    pub deadline:   i64,
    pub bid_amount: u64,
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

        let buf = &mut &data[8..];
        match ExpressRelayMetadata::deserialize(buf) {
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
            Pubkey::from_str("PytERJFhAKuNNuaiXkApLfWzwNwSNDACpigT3LwQfou")
                .expect("Failed to parse express relay pubkey")
        }
    }

    pub fn get_submit_bid_instruction(
        params: GetSubmitBidInstructionParams,
    ) -> Result<Instruction, ClientError> {
        let express_relay_pid = Self::get_express_relay_pid(params.chain_id);
        let config_router = Pubkey::find_program_address(
            &[SEED_CONFIG_ROUTER, params.router.as_ref()],
            &express_relay_pid,
        )
        .0;
        let express_relay_metadata =
            Pubkey::find_program_address(&[SEED_METADATA], &express_relay_pid).0;

        let accounts = vec![
            AccountMeta::new(params.searcher, true),
            AccountMeta::new_readonly(params.relayer_signer, true),
            AccountMeta::new_readonly(params.permission, false),
            AccountMeta::new(params.router, false),
            AccountMeta::new_readonly(config_router, false),
            AccountMeta::new(express_relay_metadata, false),
            AccountMeta::new(params.fee_receiver_relayer, false),
            AccountMeta::new_readonly(system_program::ID, false),
            AccountMeta::new_readonly(sysvar::instructions::ID, false),
        ];

        let submid_bid_args = SubmitBidArgs {
            deadline:   params.deadline,
            bid_amount: params.amount,
        };

        // Submit bid discriminator
        let mut data = vec![19, 164, 237, 254, 64, 139, 237, 93];
        submid_bid_args.serialize(&mut data).map_err(|e| {
            ClientError::SvmError(format!("Failed to serialize submit bid args: {:?}", e))
        })?;

        Ok(Instruction {
            program_id: express_relay_pid,
            accounts,
            data,
        })
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
            ApiFeeToken::InputToken => (FeeToken::Input, mint_input, input_token_program),
            ApiFeeToken::OutputToken => (FeeToken::Output, mint_output, output_token_program),
        };
        let searcher_input_ta = get_associated_token_address_with_program_id(
            &params.searcher,
            &mint_input,
            &input_token_program,
        );
        let searcher_output_ta = get_associated_token_address_with_program_id(
            &params.searcher,
            &mint_output,
            &output_token_program,
        );
        let trader_input_ata = get_associated_token_address_with_program_id(
            &swap_data.trader,
            &mint_input,
            &input_token_program,
        );
        let trader_output_ata = get_associated_token_address_with_program_id(
            &swap_data.trader,
            &mint_output,
            &output_token_program,
        );
        let router_fee_receiver_ta = get_associated_token_address_with_program_id(
            &swap_data.router_account,
            &fee_token_mint,
            &fee_token_program,
        );
        let relayer_fee_receiver_ata = get_associated_token_address_with_program_id(
            &params.fee_receiver_relayer,
            &fee_token_mint,
            &fee_token_program,
        );
        let express_relay_metadata = &Pubkey::find_program_address(
            &[SEED_METADATA],
            &Self::get_express_relay_pid(chain_id.clone()),
        )
        .0;
        let express_relay_fee_receiver_ata = get_associated_token_address_with_program_id(
            express_relay_metadata,
            &fee_token_mint,
            &fee_token_program,
        );

        let accounts = vec![
            AccountMeta::new_readonly(params.searcher, true),
            AccountMeta::new_readonly(swap_data.trader, true),
            AccountMeta::new(searcher_input_ta, false),
            AccountMeta::new(searcher_output_ta, false),
            AccountMeta::new(trader_input_ata, false),
            AccountMeta::new(trader_output_ata, false),
            AccountMeta::new(router_fee_receiver_ta, false),
            AccountMeta::new(relayer_fee_receiver_ata, false),
            AccountMeta::new(express_relay_fee_receiver_ata, false),
            AccountMeta::new_readonly(mint_input, false),
            AccountMeta::new_readonly(mint_output, false),
            AccountMeta::new_readonly(fee_token_mint, false),
            AccountMeta::new_readonly(input_token_program, false),
            AccountMeta::new_readonly(output_token_program, false),
            AccountMeta::new_readonly(fee_token_program, false),
            AccountMeta::new_readonly(*express_relay_metadata, false),
        ];

        let swap_args = SwapArgs {
            deadline: params.deadline,
            amount_input,
            amount_output,
            referral_fee_bps: swap_data.referral_fee_bps,
            fee_token,
        };

        // Swap descriminator
        let mut data = vec![248, 198, 158, 145, 225, 117, 135, 200];
        swap_args.serialize(&mut data).map_err(|e| {
            ClientError::SvmError(format!("Failed to serialize swap args: {:?}", e))
        })?;

        Ok(Instruction {
            program_id: Self::get_express_relay_pid(chain_id),
            accounts,
            data,
        })
    }
}
