use {
    super::helpers::get_accounting_key,
    crate::express_relay::helpers::{
        get_config_router_key,
        get_express_relay_metadata_key,
    },
    anchor_lang::{
        InstructionData,
        ToAccountMetas,
    },
    dummy::accounts::DoNothing,
    solana_sdk::{
        instruction::Instruction,
        pubkey::Pubkey,
        signature::Keypair,
        signer::Signer,
        system_program::id as system_program_id,
        sysvar::instructions::id as sysvar_instructions_id,
    },
};

pub fn do_nothing_instruction(
    payer: &Keypair,
    permission_key: Pubkey,
    router: Pubkey,
) -> Instruction {
    let express_relay_metadata = get_express_relay_metadata_key();
    let config_router = get_config_router_key(router);
    let accounting = get_accounting_key();
    Instruction {
        program_id: dummy::ID,
        data:       dummy::instruction::DoNothing {}.data(),
        accounts:   DoNothing {
            payer: payer.pubkey(),
            express_relay: express_relay::ID,
            express_relay_metadata,
            sysvar_instructions: sysvar_instructions_id(),
            permission: permission_key,
            router,
            config_router,
            accounting,
            system_program: system_program_id(),
        }
        .to_account_metas(None),
    }
}
