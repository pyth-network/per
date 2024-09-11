use {
    super::helpers::{
        get_config_router_key,
        get_express_relay_metadata_key,
    },
    anchor_lang::{
        InstructionData,
        ToAccountMetas,
    },
    express_relay::{
        accounts::SetRouterSplit,
        SetRouterSplitArgs,
    },
    solana_sdk::{
        instruction::Instruction,
        pubkey::Pubkey,
        signature::Keypair,
        signer::Signer,
        system_program,
    },
};

pub fn set_router_split_instruction(
    admin: &Keypair,
    router: Pubkey,
    split_router: u64,
) -> Instruction {
    let express_relay_metadata = get_express_relay_metadata_key();
    let config_router = get_config_router_key(router);
    Instruction {
        program_id: express_relay::id(),
        data:       express_relay::instruction::SetRouterSplit {
            data: SetRouterSplitArgs { split_router },
        }
        .data(),
        accounts:   SetRouterSplit {
            admin: admin.pubkey(),
            config_router,
            express_relay_metadata,
            router,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    }
}
