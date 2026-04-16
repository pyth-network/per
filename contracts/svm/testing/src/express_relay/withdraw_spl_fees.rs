use {
    super::helpers::get_express_relay_metadata_key,
    anchor_lang::{
        InstructionData,
        ToAccountMetas,
    },
    express_relay::accounts::WithdrawSplFees,
    solana_sdk::{
        instruction::Instruction,
        pubkey::Pubkey,
        signature::Keypair,
        signer::Signer,
    },
};

pub fn withdraw_spl_fees_instruction(
    admin: &Keypair,
    express_relay_fee_receiver_ata: Pubkey,
    fee_receiver_admin_ta: Pubkey,
    mint_fee: Pubkey,
    token_program_fee: Pubkey,
) -> Instruction {
    let express_relay_metadata = get_express_relay_metadata_key();

    Instruction {
        program_id: express_relay::id(),
        data:       express_relay::instruction::WithdrawSplFees {}.data(),
        accounts:   WithdrawSplFees {
            admin: admin.pubkey(),
            express_relay_metadata,
            express_relay_fee_receiver_ata,
            fee_receiver_admin_ta,
            mint_fee,
            token_program_fee,
        }
        .to_account_metas(None),
    }
}
