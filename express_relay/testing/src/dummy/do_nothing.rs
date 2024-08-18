use anchor_lang::{InstructionData, ToAccountMetas};
use solana_sdk::{instruction::Instruction, signature::Keypair, signer::Signer, pubkey::Pubkey, sysvar::instructions::id as sysvar_instructions_id};
use dummy::accounts::DoNothing;

pub fn get_do_nothing_instruction(
    payer: &Keypair,
    permission_key: Pubkey,
) -> Instruction {
    let do_nothing_ix = Instruction {
        program_id: dummy::ID,
        data: dummy::instruction::DoNothing {}.data(),
        accounts: DoNothing {
            payer: payer.pubkey(),
            express_relay: express_relay::ID,
            sysvar_instructions: sysvar_instructions_id(),
            permission: permission_key,
            protocol: dummy::ID,
        }
        .to_account_metas(None),
    };

    return do_nothing_ix;
}
