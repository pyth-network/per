use {
    crate::{
        accounts,
        instruction,
        InitializeArgs,
        ID as EXPRESS_RELAY_PID,
        SEED_METADATA,
    },
    anchor_lang::{
        prelude::*,
        solana_program::instruction::Instruction,
        system_program,
        InstructionData,
    },
};

/// Test helper method to create an instruction to initialize the express relay program.
/// Should be able to sign transactions with the secret keys of the provided payer and `relayer_signer`.
/// The fee split is set to 100% for the router, since fee payments to relayer are not important for the integrating program's tests.
/// Instead it is more important for the integrating program to ensure their router account has enough rent to avoid `InsufficientRent` error.
pub fn create_initialize_express_relay_ix(
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
