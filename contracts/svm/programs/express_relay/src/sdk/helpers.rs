use {
    crate::{
        accounts,
        instruction,
        ExpressRelayMetadata,
        FeeToken,
        SubmitBidArgs,
        SwapArgs,
        ID as EXPRESS_RELAY_PID,
        SEED_CONFIG_ROUTER,
        SEED_METADATA,
    },
    anchor_lang::{
        prelude::*,
        solana_program::{
            instruction::Instruction,
            sysvar::instructions as sysvar_instructions,
        },
        system_program,
        InstructionData,
    },
    anchor_spl::token::spl_token,
};

/// Creates and adds to the provided instructions a `SubmitBid` instruction.
#[allow(clippy::too_many_arguments)]
pub fn add_express_relay_submit_bid_instruction(
    ixs: &mut Vec<Instruction>,
    searcher: Pubkey,
    relayer_signer: Pubkey,
    fee_receiver_relayer: Pubkey,
    permission: Pubkey,
    router: Pubkey,
    bid_amount: u64,
    deadline: i64,
) -> Vec<Instruction> {
    let ix_submit_bid = create_submit_bid_instruction(
        searcher,
        relayer_signer,
        fee_receiver_relayer,
        permission,
        router,
        deadline,
        bid_amount,
    );
    ixs.push(ix_submit_bid);

    ixs.to_vec()
}

pub fn create_submit_bid_instruction(
    searcher: Pubkey,
    relayer_signer: Pubkey,
    fee_receiver_relayer: Pubkey,
    permission: Pubkey,
    router: Pubkey,
    deadline: i64,
    bid_amount: u64,
) -> Instruction {
    let config_router =
        Pubkey::find_program_address(&[SEED_CONFIG_ROUTER, router.as_ref()], &EXPRESS_RELAY_PID).0;
    let express_relay_metadata =
        Pubkey::find_program_address(&[SEED_METADATA], &EXPRESS_RELAY_PID).0;

    let accounts_submit_bid = accounts::SubmitBid {
        searcher,
        relayer_signer,
        permission,
        router,
        config_router,
        fee_receiver_relayer,
        express_relay_metadata,
        system_program: system_program::ID,
        sysvar_instructions: sysvar_instructions::ID,
    }
    .to_account_metas(None);
    let data_submit_bid = instruction::SubmitBid {
        data: SubmitBidArgs {
            deadline,
            bid_amount,
        },
    }
    .data();

    Instruction {
        program_id: EXPRESS_RELAY_PID,
        accounts:   accounts_submit_bid,
        data:       data_submit_bid,
    }
}

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
    let express_relay_metadata =
        Pubkey::find_program_address(&[SEED_METADATA], &EXPRESS_RELAY_PID).0;

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
        searcher_input_ta: searcher_input_ta.unwrap_or(
            anchor_spl::associated_token::get_associated_token_address(&searcher, &mint_input),
        ),
        searcher_output_ta: searcher_output_ta.unwrap_or(
            anchor_spl::associated_token::get_associated_token_address(&searcher, &mint_output),
        ),
        trader_input_ata: anchor_spl::associated_token::get_associated_token_address(
            &trader,
            &mint_input,
        ),
        trader_output_ata: anchor_spl::associated_token::get_associated_token_address(
            &trader,
            &mint_output,
        ),
        router_fee_receiver_ta,
        relayer_fee_receiver_ata: anchor_spl::associated_token::get_associated_token_address(
            &fee_receiver_relayer,
            &mint_fee,
        ),
        express_relay_fee_receiver_ata: anchor_spl::associated_token::get_associated_token_address(
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
    let data_submit_bid = instruction::Swap { data: swap_args }.data();

    Instruction {
        program_id: EXPRESS_RELAY_PID,
        accounts:   accounts_submit_bid,
        data:       data_submit_bid,
    }
}


pub fn deserialize_metadata(data: Vec<u8>) -> Result<ExpressRelayMetadata> {
    let buf = &mut &data[..];
    match ExpressRelayMetadata::try_deserialize(buf) {
        Ok(metadata) => Ok(metadata),
        Err(_) => Err(ProgramError::InvalidAccountData.into()),
    }
}
