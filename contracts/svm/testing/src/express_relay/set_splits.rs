use {
    super::helpers::get_express_relay_metadata_key,
    anchor_lang::{
        InstructionData,
        ToAccountMetas,
    },
    express_relay::{
        accounts::SetSplits,
        SetSplitsArgs,
    },
    solana_sdk::{
        instruction::Instruction,
        signature::Keypair,
        signer::Signer,
    },
};

pub fn set_splits_instruction(
    admin: &Keypair,
    split_router_default: u64,
    split_relayer: u64,
) -> Instruction {
    let express_relay_metadata = get_express_relay_metadata_key();

    Instruction {
        program_id: express_relay::id(),
        data:       express_relay::instruction::SetSplits {
            data: SetSplitsArgs {
                split_router_default,
                split_relayer,
            },
        }
        .data(),
        accounts:   SetSplits {
            admin: admin.pubkey(),
            express_relay_metadata,
        }
        .to_account_metas(None),
    }
}
