use {
    super::helpers::get_express_relay_metadata_key,
    anchor_lang::{
        InstructionData,
        ToAccountMetas,
    },
    express_relay::accounts::SetSecondaryRelayer,
    solana_sdk::{
        instruction::Instruction,
        pubkey::Pubkey,
        signature::Keypair,
        signer::Signer,
    },
};

pub fn set_secondary_relayer_instruction(
    admin: &Keypair,
    secondary_relayer_signer: Pubkey,
) -> Instruction {
    let express_relay_metadata = get_express_relay_metadata_key();

    Instruction {
        program_id: express_relay::id(),
        data:       express_relay::instruction::SetSecondaryRelayer {}.data(),
        accounts:   SetSecondaryRelayer {
            admin: admin.pubkey(),
            express_relay_metadata,
            secondary_relayer_signer,
        }
        .to_account_metas(None),
    }
}
