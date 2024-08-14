use anchor_lang::{InstructionData, ToAccountMetas};
use solana_sdk::{instruction::Instruction, signature::Keypair, signer::Signer, system_program};
use dummy::accounts::Initialize;

use crate::express_relay::helpers::get_protocol_fee_receiver_key;

pub fn get_initialize_instruction(
    payer: &Keypair
) -> Instruction {
    let fee_receiver_express_relay = get_protocol_fee_receiver_key(dummy::id());
    let system_program_pk = system_program::ID;

    let initialize_ix = Instruction {
        program_id: dummy::id(),
        data: dummy::instruction::Initialize {}.data(),
        accounts: Initialize {
            payer: payer.pubkey(),
            fee_receiver_express_relay: fee_receiver_express_relay,
            system_program: system_program_pk,
        }
        .to_account_metas(None),
    };

    return initialize_ix;
}
