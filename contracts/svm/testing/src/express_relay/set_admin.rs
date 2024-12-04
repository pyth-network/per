use {
    super::helpers::get_express_relay_metadata_key,
    anchor_lang::{
        InstructionData,
        ToAccountMetas,
    },
    express_relay::accounts::SetAdmin,
    solana_sdk::{
        instruction::Instruction,
        pubkey::Pubkey,
        signature::Keypair,
        signer::Signer,
    },
};

pub fn set_admin_instruction(admin: &Keypair, admin_new: Pubkey) -> Instruction {
    let express_relay_metadata = get_express_relay_metadata_key();

    Instruction {
        program_id: express_relay::id(),
        data:       express_relay::instruction::SetAdmin {}.data(),
        accounts:   SetAdmin {
            admin: admin.pubkey(),
            express_relay_metadata,
            admin_new,
        }
        .to_account_metas(None),
    }
}
