use {
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
        sysvar::instructions::id as sysvar_instructions_id,
    },
};

pub fn do_nothing_instruction(
    payer: &Keypair,
    permission_key: Pubkey,
    router: Pubkey,
) -> Instruction {
    Instruction {
        program_id: dummy::ID,
        data:       dummy::instruction::DoNothing {}.data(),
        accounts:   DoNothing {
            payer: payer.pubkey(),
            express_relay: express_relay::ID,
            sysvar_instructions: sysvar_instructions_id(),
            permission: permission_key,
            router,
        }
        .to_account_metas(None),
    }
}
