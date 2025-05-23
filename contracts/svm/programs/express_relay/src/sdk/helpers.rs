use {
    crate::{
        accounts,
        instruction,
        ExpressRelayMetadata,
        FeeToken,
        SubmitBidArgs,
        SwapV2Args,
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
    anchor_spl::associated_token::{
        self,
        get_associated_token_address_with_program_id,
    },
};

/// Creates and adds to the provided instructions a `SubmitBid` instruction.
#[allow(clippy::too_many_arguments)]
pub fn add_submit_bid_instruction(
    ixs: &mut Vec<Instruction>,
    express_relay_pid: Pubkey,
    searcher: Pubkey,
    relayer_signer: Pubkey,
    fee_receiver_relayer: Pubkey,
    permission: Pubkey,
    router: Pubkey,
    bid_amount: u64,
    deadline: i64,
) -> Vec<Instruction> {
    let ix_submit_bid = create_submit_bid_instruction(
        express_relay_pid,
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

#[allow(clippy::too_many_arguments)]
pub fn create_submit_bid_instruction(
    express_relay_pid: Pubkey,
    searcher: Pubkey,
    relayer_signer: Pubkey,
    fee_receiver_relayer: Pubkey,
    permission: Pubkey,
    router: Pubkey,
    deadline: i64,
    bid_amount: u64,
) -> Instruction {
    let config_router =
        Pubkey::find_program_address(&[SEED_CONFIG_ROUTER, router.as_ref()], &express_relay_pid).0;
    let express_relay_metadata =
        Pubkey::find_program_address(&[SEED_METADATA], &express_relay_pid).0;

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
        program_id: express_relay_pid,
        accounts:   accounts_submit_bid,
        data:       data_submit_bid,
    }
}

#[allow(clippy::too_many_arguments)]
pub fn create_swap_instruction(
    express_relay_pid: Pubkey,
    searcher: Pubkey,
    user: Pubkey,
    searcher_ta_mint_searcher: Option<Pubkey>,
    searcher_ta_mint_user: Option<Pubkey>,
    router_fee_receiver_ta: Pubkey,
    fee_receiver_relayer: Pubkey,
    mint_searcher: Pubkey,
    mint_user: Pubkey,
    token_program_searcher: Pubkey,
    token_program_user: Pubkey,
    swap_args: SwapV2Args,
    relayer_signer: Pubkey,
) -> Instruction {
    let express_relay_metadata =
        Pubkey::find_program_address(&[SEED_METADATA], &express_relay_pid).0;

    let (mint_fee, token_program_fee) = match swap_args.fee_token {
        FeeToken::Searcher => (mint_searcher, token_program_searcher),
        FeeToken::User => (mint_user, token_program_user),
    };

    let accounts_swap = accounts::Swap {
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
        user_ata_mint_user: get_associated_token_address_with_program_id(
            &user,
            &mint_user,
            &token_program_user,
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
        relayer_signer,
    }
    .to_account_metas(None);
    let data_submit_bid = instruction::SwapV2 { data: swap_args }.data();

    Instruction {
        program_id: express_relay_pid,
        accounts:   accounts_swap,
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

/// Creates CreateIdempotent instruction
pub fn create_associated_token_account_idempotent(
    funding_address: &Pubkey,
    wallet_address: &Pubkey,
    token_mint_address: &Pubkey,
    token_program_id: &Pubkey,
) -> Instruction {
    build_associated_token_account_instruction(
        funding_address,
        wallet_address,
        token_mint_address,
        token_program_id,
        1, // AssociatedTokenAccountInstruction::CreateIdempotent
    )
}

fn build_associated_token_account_instruction(
    funding_address: &Pubkey,
    wallet_address: &Pubkey,
    token_mint_address: &Pubkey,
    token_program_id: &Pubkey,
    instruction: u8,
) -> Instruction {
    let associated_account_address = get_associated_token_address_with_program_id(
        wallet_address,
        token_mint_address,
        token_program_id,
    );
    // safety check, assert if not a creation instruction, which is only 0 or 1
    assert!(instruction <= 1);
    Instruction {
        program_id: associated_token::ID,
        accounts:   vec![
            AccountMeta::new(*funding_address, true),
            AccountMeta::new(associated_account_address, false),
            AccountMeta::new_readonly(*wallet_address, false),
            AccountMeta::new_readonly(*token_mint_address, false),
            AccountMeta::new_readonly(system_program::ID, false),
            AccountMeta::new_readonly(*token_program_id, false),
        ],
        data:       vec![instruction],
    }
}
