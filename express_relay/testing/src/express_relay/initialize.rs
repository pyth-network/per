use anchor_lang::{InstructionData, ToAccountMetas};
use solana_sdk::{instruction::Instruction, pubkey::Pubkey, signature::Keypair, signer::Signer, system_program};
use express_relay::{accounts::Initialize, InitializeArgs};

use super::helpers::get_express_relay_metadata_key;

pub fn get_initialize_instruction(
    payer: &Keypair,
    admin: Pubkey,
    relayer_signer: Pubkey,
    fee_receiver_relayer: Pubkey,
    split_router_default: u64,
    split_relayer: u64
) -> Instruction {
    let express_relay_metadata = get_express_relay_metadata_key();

    let initialize_ix = Instruction {
        program_id: express_relay::id(),
        data:
        express_relay::instruction::Initialize {
            data: InitializeArgs {
                split_router_default: split_router_default,
                split_relayer: split_relayer,
            }
        }.data(),
        accounts: Initialize {
            payer: payer.pubkey(),
            express_relay_metadata: express_relay_metadata,
            admin: admin,
            relayer_signer: relayer_signer,
            fee_receiver_relayer: fee_receiver_relayer,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    };

    return initialize_ix;
}
