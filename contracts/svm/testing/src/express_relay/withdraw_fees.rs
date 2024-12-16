use {
    super::helpers::get_express_relay_metadata_key,
    anchor_lang::{
        InstructionData,
        ToAccountMetas,
    },
    express_relay::accounts::WithdrawFees,
    solana_sdk::{
        instruction::Instruction,
        pubkey::Pubkey,
        signature::Keypair,
        signer::Signer,
    },
};

pub fn withdraw_fees_instruction(admin: &Keypair, fee_receiver_admin: Pubkey) -> Instruction {
    let express_relay_metadata = get_express_relay_metadata_key();

    Instruction {
        program_id: express_relay::id(),
        data:       express_relay::instruction::WithdrawFees {}.data(),
        accounts:   WithdrawFees {
            admin: admin.pubkey(),
            fee_receiver_admin,
            express_relay_metadata,
        }
        .to_account_metas(None),
    }
}
