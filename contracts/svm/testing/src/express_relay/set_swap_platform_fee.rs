use {
    super::helpers::get_express_relay_metadata_key,
    anchor_lang::{
        InstructionData,
        ToAccountMetas,
    },
    express_relay::{
        accounts::SetSplits,
        SetSwapPlatformFeeArgs,
    },
    solana_sdk::{
        instruction::Instruction,
        signature::Keypair,
        signer::Signer,
    },
};

pub fn set_swap_platform_fee_instruction(
    admin: &Keypair,
    swap_platform_fee_bps: u64,
) -> Instruction {
    let express_relay_metadata = get_express_relay_metadata_key();

    Instruction {
        program_id: express_relay::id(),
        data:       express_relay::instruction::SetSwapPlatformFee {
            data: SetSwapPlatformFeeArgs {
                swap_platform_fee_bps,
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
