use {
    crate::{
        accounts,
        instruction,
        InitializeArgs,
        SubmitBidArgs,
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
};

// This file contains setup helpers for integrating programs to set up express relay for Rust-based tests

// Helper method to create an instruction to initialize the express relay program
// Should be able to sign transactions with the secret keys of the provided payer and relayer_signer
// The fee split is set to 100% for the router, since fee payments to relayer are not important for the integrating program's tests
// Instead it is more relevant for the integrating program to ensure their router account has enough rent to avoid InsufficientRent error
pub fn create_initialize_express_relay_ix<'info>(
    payer: Pubkey,
    admin: Pubkey,
    relayer_signer: Pubkey,
    fee_receiver_relayer: Pubkey,
) -> Instruction {
    let express_relay_metadata =
        Pubkey::find_program_address(&[SEED_METADATA], &EXPRESS_RELAY_PID).0;

    let split_router_default = 10000;
    let split_relayer = 0;

    let accounts_initialize = accounts::Initialize {
        payer,
        express_relay_metadata,
        admin,
        relayer_signer,
        fee_receiver_relayer,
        system_program: system_program::ID,
    }
    .to_account_metas(None);
    let data_initialize = instruction::Initialize {
        data: InitializeArgs {
            split_router_default,
            split_relayer,
        },
    }
    .data();

    Instruction {
        program_id: EXPRESS_RELAY_PID,
        accounts:   accounts_initialize,
        data:       data_initialize,
    }
}

// Creates and adds a SubmitBid instruction to the provided instructions
pub fn add_express_relay_submit_bid_instruction(
    ixs: &mut Vec<Instruction>,
    searcher: Pubkey,
    relayer_signer: Pubkey,
    fee_receiver_relayer: Pubkey,
    permission: Pubkey,
    router: Pubkey,
    bid_amount: u64,
) -> Vec<Instruction> {
    let deadline = i64::MAX;

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
    let router_config =
        Pubkey::find_program_address(&[SEED_CONFIG_ROUTER, router.as_ref()], &EXPRESS_RELAY_PID).0;
    let express_relay_metadata =
        Pubkey::find_program_address(&[SEED_METADATA], &EXPRESS_RELAY_PID).0;

    let accounts_submit_bid = accounts::SubmitBid {
        searcher,
        relayer_signer,
        permission,
        router,
        router_config,
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
