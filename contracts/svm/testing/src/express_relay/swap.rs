use {
    super::helpers::get_express_relay_metadata_key,
    anchor_lang::{
        InstructionData,
        ToAccountMetas,
    },
    anchor_spl::{
        associated_token::{
            get_associated_token_address, get_associated_token_address_with_program_id, spl_associated_token_account::instruction::create_associated_token_account_idempotent
        },
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

    let token_program_input = token_program_input.unwrap_or(spl_token::ID);
    let token_program_output = token_program_output.unwrap_or(spl_token::ID);

    let token_program_fee = match swap_args.fee_token {
        FeeToken::Input => token_program_input,
        FeeToken::Output => token_program_output,
    };
    let accounts_submit_bid = accounts::Swap {
        searcher,
        trader,
        searcher_input_ta: searcher_input_ta
            .unwrap_or(get_associated_token_address_with_program_id(&searcher, &mint_input, &token_program_input)),
        searcher_output_ta: searcher_output_ta
            .unwrap_or(get_associated_token_address_with_program_id(&searcher, &mint_output, &token_program_output)),
        trader_input_ata: get_associated_token_address_with_program_id(&trader, &mint_input, &token_program_input),
        trader_output_ata: get_associated_token_address_with_program_id(&trader, &mint_output, &token_program_output),
        router_fee_receiver_ta,
        relayer_fee_receiver_ata: get_associated_token_address_with_program_id(&fee_receiver_relayer, &mint_fee, &token_program_fee),
        express_relay_fee_receiver_ata: get_associated_token_address_with_program_id(
            &express_relay_metadata,
            &mint_fee,
            &token_program_fee,
        ),
        mint_input,
        mint_output,
        mint_fee,
        token_program_input,
        token_program_output,
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

#[allow(clippy::too_many_arguments)]
pub fn build_swap_instructions(
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
) -> Vec<Instruction> {
    let mut instructions: Vec<Instruction> = vec![];

    let token_program_input = token_program_input.unwrap_or(spl_token::ID);
    let token_program_output = token_program_output.unwrap_or(spl_token::ID);
    let mint_fee = match swap_args.fee_token {
        FeeToken::Input => mint_input,
        FeeToken::Output => mint_output,
    };
    let token_program_fee = match swap_args.fee_token {
        FeeToken::Input => token_program_input,
        FeeToken::Output => token_program_output,
    };

    if searcher_output_ta.is_none() {
        instructions.push(create_associated_token_account_idempotent(
            &searcher,
            &searcher,
            &mint_output,
            &token_program_output,
        ));
    }
    instructions.push(create_associated_token_account_idempotent(
        &searcher,
        &trader,
        &mint_input,
        &token_program_input,
    ));
    instructions.push(create_associated_token_account_idempotent(
        &searcher,
        &fee_receiver_relayer,
        &mint_fee,
        &token_program_fee,
    ));
    instructions.push(create_associated_token_account_idempotent(
        &searcher,
        &get_express_relay_metadata_key(),
        &mint_fee,
        &token_program_fee,
    ));


    instructions.push(create_swap_instruction(
        searcher,
        trader,
        searcher_input_ta,
        searcher_output_ta,
        router_fee_receiver_ta,
        fee_receiver_relayer,
        mint_input,
        mint_output,
        Some(token_program_input),
        Some(token_program_output),
        swap_args,
    ));

    instructions
}
