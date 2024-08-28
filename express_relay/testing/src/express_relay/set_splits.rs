use anchor_lang::{InstructionData, ToAccountMetas};
use solana_sdk::{instruction::Instruction, signature::Keypair, signer::Signer};
use express_relay::{accounts::SetSplits, SetSplitsArgs};

use super::helpers::get_express_relay_metadata_key;

pub fn get_set_splits_instruction(
    admin: &Keypair,
    split_protocol_default: u64,
    split_relayer: u64
) -> Instruction {
    let express_relay_metadata = get_express_relay_metadata_key();

    let set_splits_ix = Instruction {
        program_id: express_relay::id(),
        data:
        express_relay::instruction::SetSplits {
            data: SetSplitsArgs {
                split_protocol_default,
                split_relayer,
            }
        }.data(),
        accounts: SetSplits {
            admin: admin.pubkey(),
            express_relay_metadata: express_relay_metadata,
        }
        .to_account_metas(None),
    };

    return set_splits_ix;
}
