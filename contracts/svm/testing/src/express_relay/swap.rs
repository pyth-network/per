use {
    super::helpers::get_express_relay_metadata_key,
    anchor_lang::{
        InstructionData,
        ToAccountMetas,
    },
    anchor_spl::{
        associated_token::get_associated_token_address,
        token::spl_token,
    },
    express_relay::{
        accounts::{
            self,
        },
        instruction::Swap,
        FeeToken,
        SwapArgs,
    },
    solana_sdk::{
        instruction::Instruction,
        pubkey::Pubkey,
    },
};

#[allow(clippy::too_many_arguments)]
pub fn create_swap_instruction(
    searcher: Pubkey,
    trader: Pubkey,
    searcher_input_ta: Option<Pubkey>,
    searcher_output_ta: Option<Pubkey>,
    router_fee_receiver_ta: Pubkey,
    fee_receiver_relayer: Pubkey,
    mint_input: Pubkey,
    mint_output: Pubkey,
    token_program_input: Option<Pubkey>,
    token_program_output: Option<Pubkey>,
    swap_args: SwapArgs,
) -> Instruction {
    let express_relay_metadata = get_express_relay_metadata_key();

    let mint_fee = match swap_args.fee_token {
        FeeToken::Input => mint_input,
        FeeToken::Output => mint_output,
    };
    let token_program_fee = match swap_args.fee_token {
        FeeToken::Input => token_program_input.unwrap_or(spl_token::ID),
        FeeToken::Output => token_program_output.unwrap_or(spl_token::ID),
    };
    let accounts_submit_bid = accounts::Swap {
        searcher,
        trader,
        searcher_input_ta: searcher_input_ta
            .unwrap_or(get_associated_token_address(&searcher, &mint_input)),
        searcher_output_ta: searcher_output_ta
            .unwrap_or(get_associated_token_address(&searcher, &mint_output)),
        trader_input_ata: get_associated_token_address(&trader, &mint_input),
        trader_output_ata: get_associated_token_address(&trader, &mint_output),
        router_fee_receiver_ta,
        relayer_fee_receiver_ata: get_associated_token_address(&fee_receiver_relayer, &mint_fee),
        express_relay_fee_receiver_ata: get_associated_token_address(
            &express_relay_metadata,
            &mint_fee,
        ),
        mint_input,
        mint_output,
        mint_fee,
        token_program_input: token_program_input.unwrap_or(spl_token::ID),
        token_program_output: token_program_output.unwrap_or(spl_token::ID),
        token_program_fee,
        express_relay_metadata,
    }
    .to_account_metas(None);

    Instruction {
        program_id: express_relay::ID,
        accounts:   accounts_submit_bid,
        data:       Swap { data: swap_args }.data(),
    }
}
