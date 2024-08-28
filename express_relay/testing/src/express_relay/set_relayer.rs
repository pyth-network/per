use anchor_lang::{InstructionData, ToAccountMetas};
use solana_sdk::{instruction::Instruction, pubkey::Pubkey, signature::Keypair, signer::Signer};
use express_relay::accounts::SetRelayer;

use super::helpers::get_express_relay_metadata_key;

pub fn get_set_relayer_instruction(
    admin: &Keypair,
    relayer_signer: Pubkey,
    fee_receiver_relayer: Pubkey
) -> Instruction {
    let express_relay_metadata = get_express_relay_metadata_key();

    let set_relayer_ix = Instruction {
        program_id: express_relay::id(),
        data:
        express_relay::instruction::SetRelayer {}.data(),
        accounts: SetRelayer {
            admin: admin.pubkey(),
            express_relay_metadata: express_relay_metadata,
            relayer_signer: relayer_signer,
            fee_receiver_relayer: fee_receiver_relayer,
        }
        .to_account_metas(None),
    };

    return set_relayer_ix;
}
