use {
    super::helpers::get_express_relay_metadata_key,
    anchor_lang::{
        InstructionData,
        ToAccountMetas,
    },
    anchor_spl::{
        associated_token::{
            get_associated_token_address_with_program_id,
            spl_associated_token_account::instruction::create_associated_token_account_idempotent,
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

/// Builds a swap instruction.
/// If provides two overrides, `user_ata_mint_user_override` and `mint_fee_override`, that may result in an invalid instruction and are meant to be used for testing.
#[allow(clippy::too_many_arguments)]
pub fn create_swap_instruction(
    searcher: Pubkey,
    user: Pubkey,
    searcher_ta_mint_searcher: Option<Pubkey>,
    searcher_ta_mint_user: Option<Pubkey>,
    router_fee_receiver_ta: Pubkey,
    fee_receiver_relayer: Pubkey,
    mint_searcher: Pubkey,
    mint_user: Pubkey,
    token_program_searcher: Option<Pubkey>,
    token_program_user: Option<Pubkey>,
    swap_args: SwapArgs,
    user_ata_mint_user_override: Option<Pubkey>,
    mint_fee_override: Option<Pubkey>,
) -> Instruction {
    let express_relay_metadata = get_express_relay_metadata_key();

    let mint_fee = mint_fee_override.unwrap_or(match swap_args.fee_token {
        FeeToken::Searcher => mint_searcher,
        FeeToken::User => mint_user,
    });

    let token_program_searcher = token_program_searcher.unwrap_or(spl_token::ID);
    let token_program_user = token_program_user.unwrap_or(spl_token::ID);

    let token_program_fee = match swap_args.fee_token {
        FeeToken::Searcher => token_program_searcher,
        FeeToken::User => token_program_user,
    };
    let accounts_submit_bid = accounts::Swap {
        searcher,
        user,
        searcher_ta_mint_searcher: searcher_ta_mint_searcher.unwrap_or(
            get_associated_token_address_with_program_id(
                &searcher,
                &mint_searcher,
                &token_program_searcher,
            ),
        ),
        searcher_ta_mint_user: searcher_ta_mint_user.unwrap_or(
            get_associated_token_address_with_program_id(
                &searcher,
                &mint_user,
                &token_program_user,
            ),
        ),
        user_ata_mint_searcher: get_associated_token_address_with_program_id(
            &user,
            &mint_searcher,
            &token_program_searcher,
        ),
        user_ata_mint_user: user_ata_mint_user_override.unwrap_or(
            get_associated_token_address_with_program_id(&user, &mint_user, &token_program_user),
        ),
        router_fee_receiver_ta,
        relayer_fee_receiver_ata: get_associated_token_address_with_program_id(
            &fee_receiver_relayer,
            &mint_fee,
            &token_program_fee,
        ),
        express_relay_fee_receiver_ata: get_associated_token_address_with_program_id(
            &express_relay_metadata,
            &mint_fee,
            &token_program_fee,
        ),
        mint_searcher,
        mint_user,
        mint_fee,
        token_program_searcher,
        token_program_user,
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

/// Builds a set of instructions to perform a swap, including creating the associated token accounts.
/// If provides two overrides, `user_ata_mint_user_override` and `mint_fee_override`, that may result in invalid instructions and are meant to be used for testing.
#[allow(clippy::too_many_arguments)]
pub fn build_swap_instructions(
    searcher: Pubkey,
    user: Pubkey,
    searcher_ta_mint_searcher: Option<Pubkey>,
    searcher_ta_mint_user: Option<Pubkey>,
    router_fee_receiver_ta: Pubkey,
    fee_receiver_relayer: Pubkey,
    mint_searcher: Pubkey,
    mint_user: Pubkey,
    token_program_searcher: Option<Pubkey>,
    token_program_user: Option<Pubkey>,
    swap_args: SwapArgs,
    user_ata_mint_user_override: Option<Pubkey>,
    mint_fee_override: Option<Pubkey>,
) -> Vec<Instruction> {
    let mut instructions: Vec<Instruction> = vec![];

    let token_program_searcher = token_program_searcher.unwrap_or(spl_token::ID);
    let token_program_user = token_program_user.unwrap_or(spl_token::ID);
    let mint_fee = mint_fee_override.unwrap_or(match swap_args.fee_token {
        FeeToken::Searcher => mint_searcher,
        FeeToken::User => mint_user,
    });
    let token_program_fee = match swap_args.fee_token {
        FeeToken::Searcher => token_program_searcher,
        FeeToken::User => token_program_user,
    };

    if searcher_ta_mint_user.is_none() {
        instructions.push(create_associated_token_account_idempotent(
            &searcher,
            &searcher,
            &mint_user,
            &token_program_user,
        ));
    }
    instructions.push(create_associated_token_account_idempotent(
        &searcher,
        &user,
        &mint_searcher,
        &token_program_searcher,
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
        user,
        searcher_ta_mint_searcher,
        searcher_ta_mint_user,
        router_fee_receiver_ta,
        fee_receiver_relayer,
        mint_searcher,
        mint_user,
        Some(token_program_searcher),
        Some(token_program_user),
        swap_args,
        user_ata_mint_user_override,
        mint_fee_override,
    ));

    instructions
}
