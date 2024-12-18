use {
    anchor_lang::prelude::*,
    express_relay::state::{
        ConfigRouter,
        ExpressRelayMetadata,
        SEED_CONFIG_ROUTER,
        SEED_METADATA,
    },
    solana_sdk::pubkey::Pubkey,
};

pub fn get_express_relay_metadata_key() -> Pubkey {
    Pubkey::find_program_address(&[SEED_METADATA], &express_relay::id()).0
}

pub fn get_express_relay_metadata(svm: litesvm::LiteSVM) -> ExpressRelayMetadata {
    let express_relay_metadata_key = get_express_relay_metadata_key();
    let express_relay_metadata_acc = svm
        .get_account(&express_relay_metadata_key)
        .expect("Express Relay Metadata is not initialized");
    let express_relay_metadata =
        ExpressRelayMetadata::try_deserialize(&mut express_relay_metadata_acc.data.as_ref())
            .expect("Account is not of struct ExpressRelayMetadata");
    express_relay_metadata
}

pub fn get_config_router_key(router: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[SEED_CONFIG_ROUTER, router.as_ref()], &express_relay::id()).0
}

pub fn get_config_router(svm: litesvm::LiteSVM, router: Pubkey) -> Option<ConfigRouter> {
    let config_router_key = get_config_router_key(router);
    svm.get_account(&config_router_key).map(|acc| {
        ConfigRouter::try_deserialize(&mut acc.data.as_ref())
            .expect("Account is not of struct ConfigRouter")
    })
}
