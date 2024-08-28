use anchor_lang::{InstructionData, ToAccountMetas};
use solana_sdk::{instruction::Instruction, pubkey::Pubkey, signature::Keypair, signer::Signer, system_program};
use express_relay::{accounts::SetProtocolSplit, SetProtocolSplitArgs};

use super::helpers::{get_express_relay_metadata_key, get_protocol_config_key};

pub fn get_set_protocol_split_instruction(
    admin: &Keypair,
    protocol: Pubkey,
    split_protocol: u64,
) -> Instruction {
    let express_relay_metadata = get_express_relay_metadata_key();
    let protocol_config = get_protocol_config_key(protocol);

    let set_protocol_split_ix = Instruction {
        program_id: express_relay::id(),
        data: express_relay::instruction::SetProtocolSplit {
            data: SetProtocolSplitArgs {
                split_protocol: split_protocol,
            }
        }.data(),
        accounts: SetProtocolSplit {
            admin: admin.pubkey(),
            protocol_config: protocol_config,
            express_relay_metadata: express_relay_metadata,
            protocol: protocol,
            system_program: system_program::ID,
        }.to_account_metas(None),
    };

    return set_protocol_split_ix;
}
