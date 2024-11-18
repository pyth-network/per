use {
    super::helpers::{
        get_config_router_key,
        get_express_relay_metadata_key,
    },
    anchor_lang::{
        InstructionData,
        ToAccountMetas,
    },
    express_relay::{
        accounts::Swap,
        SwapArgs,
    },
    solana_sdk::{
        instruction::Instruction,
        pubkey::Pubkey,
        signature::Keypair,
        signer::Signer,
        system_program,
        sysvar::instructions::id as sysvar_instructions_id,
    },
};

pub struct TokenInfo {
    pub mint:    Keypair,
    pub amount:  u64,
    pub program: Pubkey,
}

pub struct TokenAccounts {
    pub input:  Pubkey,
    pub output: Pubkey,
}

pub struct ReferralFeeInfo {
    pub input: bool,
    pub ppm:   u64,
}

pub fn swap_instruction(
    searcher: &Keypair,
    trader: &Keypair,
    router: Pubkey,
    permission: Pubkey,
    token_info_input: TokenInfo,
    token_info_output: TokenInfo,
    token_accounts_searcher: &TokenAccounts,
    token_accounts_trader: &TokenAccounts,
    token_accounts_router: &TokenAccounts,
    referral_fee_info: ReferralFeeInfo,
) -> Instruction {
    let express_relay_metadata = get_express_relay_metadata_key();
    let config_router = get_config_router_key(router);

    Instruction {
        program_id: express_relay::ID,
        data:       express_relay::instruction::Swap {
            data: SwapArgs {
                amount_input:       token_info_input.amount,
                amount_output:      token_info_output.amount,
                referral_fee_input: referral_fee_info.input,
                referral_fee_ppm:   referral_fee_info.ppm,
            },
        }
        .data(),
        accounts:   Swap {
            searcher: searcher.pubkey(),
            trader: trader.pubkey(),
            permission,
            router,
            config_router,
            express_relay_metadata,
            mint_input: token_info_input.mint.pubkey(),
            mint_output: token_info_output.mint.pubkey(),
            ta_input_searcher: token_accounts_searcher.input,
            ta_output_searcher: token_accounts_searcher.output,
            ta_input_trader: token_accounts_trader.input,
            ta_output_trader: token_accounts_trader.output,
            ta_input_router: token_accounts_router.input,
            ta_output_router: token_accounts_router.output,
            express_relay_program: express_relay::ID,
            token_program_input: token_info_input.program,
            token_program_output: token_info_output.program,
            system_program: system_program::ID,
            sysvar_instructions: sysvar_instructions_id(),
        }
        .to_account_metas(None),
    }
}
