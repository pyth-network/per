use anchor_lang::{InstructionData, ToAccountMetas};
use solana_sdk::{instruction::Instruction, pubkey::Pubkey, signature::Keypair, signer::Signer};
use express_relay::accounts::SetAdmin;

use super::helpers::get_express_relay_metadata_key;

pub fn set_admin_instruction(
    admin: &Keypair,
    admin_new: Pubkey
) -> Instruction {
    let express_relay_metadata = get_express_relay_metadata_key();

    let set_admin_ix = Instruction {
        program_id: express_relay::id(),
        data: express_relay::instruction::SetAdmin {}.data(),
        accounts: SetAdmin {
            admin: admin.pubkey(),
            express_relay_metadata: express_relay_metadata,
            admin_new: admin_new,
        }.to_account_metas(None),
    };

    return set_admin_ix;
}
