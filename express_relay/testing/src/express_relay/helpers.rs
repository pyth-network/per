use anchor_lang::prelude::*;
use solana_sdk::pubkey::Pubkey;
use express_relay::state::{ConfigRouter, ExpressRelayMetadata, SEED_CONFIG_ROUTER, SEED_METADATA};

pub fn get_express_relay_metadata_key() -> Pubkey {
    return Pubkey::find_program_address(&[SEED_METADATA], &express_relay::id()).0;
}

pub fn get_express_relay_metadata(svm: litesvm::LiteSVM) -> ExpressRelayMetadata {
    let express_relay_metadata_key = get_express_relay_metadata_key();
    let express_relay_metadata_acc = svm.get_account(&express_relay_metadata_key).expect("Express Relay Metadata is not initialized");
    let express_relay_metadata = ExpressRelayMetadata::try_deserialize(&mut express_relay_metadata_acc.data.as_ref()).expect("Account is not of struct ExpressRelayMetadata");
    return express_relay_metadata;
}

pub fn get_router_config_key(router: Pubkey) -> Pubkey {
    return Pubkey::find_program_address(&[SEED_CONFIG_ROUTER, router.as_ref()], &express_relay::id()).0;
}

pub fn get_router_config(svm: litesvm::LiteSVM, router: Pubkey) -> Option<ConfigRouter> {
    let router_config_key = get_router_config_key(router);
    return svm.get_account(&router_config_key).map(|acc| ConfigRouter::try_deserialize(&mut acc.data.as_ref()).expect("Account is not of struct ConfigRouter"));
}
