use anchor_lang::{InstructionData, ToAccountMetas};
use solana_sdk::{instruction::Instruction, pubkey::Pubkey, signature::Keypair, signer::Signer};
use express_relay::accounts::WithdrawFees;

use super::helpers::get_express_relay_metadata_key;

pub fn get_withdraw_fees_instruction(
    admin: &Keypair,
    fee_receiver_admin: Pubkey,
) -> Instruction {
    let express_relay_metadata = get_express_relay_metadata_key();

    let withdraw_fees_ix = Instruction {
        program_id: express_relay::id(),
        data: express_relay::instruction::WithdrawFees {}.data(),
        accounts: WithdrawFees {
            admin: admin.pubkey(),
            fee_receiver_admin: fee_receiver_admin,
            express_relay_metadata: express_relay_metadata,
        }.to_account_metas(None),
    };

    return withdraw_fees_ix;
}
