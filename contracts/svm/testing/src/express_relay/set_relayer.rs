use {
    super::helpers::get_express_relay_metadata_key,
    anchor_lang::{
        InstructionData,
        ToAccountMetas,
    },
    express_relay::accounts::SetRelayer,
    solana_sdk::{
        instruction::Instruction,
        pubkey::Pubkey,
        signature::Keypair,
        signer::Signer,
    },
};

pub fn set_relayer_instruction(
    admin: &Keypair,
    relayer_signer: Pubkey,
    fee_receiver_relayer: Pubkey,
) -> Instruction {
    let express_relay_metadata = get_express_relay_metadata_key();


    Instruction {
        program_id: express_relay::id(),
        data:       express_relay::instruction::SetRelayer {}.data(),
        accounts:   SetRelayer {
            admin: admin.pubkey(),
            express_relay_metadata,
            relayer_signer,
            fee_receiver_relayer,
        }
        .to_account_metas(None),
    }
}
