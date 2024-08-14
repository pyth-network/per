use anchor_lang::prelude::*;
use solana_sdk::pubkey::Pubkey;
use express_relay::state::{ConfigProtocol, ExpressRelayMetadata, SEED_CONFIG_PROTOCOL, SEED_EXPRESS_RELAY_FEES, SEED_METADATA};

pub fn get_express_relay_metadata_key() -> Pubkey {
    return Pubkey::find_program_address(&[SEED_METADATA], &express_relay::id()).0;
}

pub fn get_express_relay_metadata(svm: litesvm::LiteSVM) -> ExpressRelayMetadata {
    let express_relay_metadata_key = get_express_relay_metadata_key();
    let express_relay_metadata_acc = svm.get_account(&express_relay_metadata_key).expect("Express Relay Metadata is not initialized");
    let express_relay_metadata = ExpressRelayMetadata::try_deserialize(&mut express_relay_metadata_acc.data.as_ref()).expect("Account is not of struct ExpressRelayMetadata");
    return express_relay_metadata;
}

pub fn get_protocol_config_key(protocol: Pubkey) -> Pubkey {
    return Pubkey::find_program_address(&[SEED_CONFIG_PROTOCOL, protocol.as_ref()], &express_relay::id()).0;
}

pub fn get_protocol_config(svm: litesvm::LiteSVM, protocol: Pubkey) -> Option<ConfigProtocol> {
    let protocol_config_key = get_protocol_config_key(protocol);
    let protocol_config_acc = svm.get_account(&protocol_config_key);
    match protocol_config_acc {
        Some(protocol_config_acc) => {
            let protocol_config = ConfigProtocol::try_deserialize(&mut protocol_config_acc.data.as_ref()).expect("Account is not of struct ConfigProtocol");
            return Some(protocol_config);
        },
        None => {
            return None;
        }
    }
}

pub fn get_protocol_fee_receiver_key(protocol: Pubkey) -> Pubkey {
    return Pubkey::find_program_address(&[SEED_EXPRESS_RELAY_FEES], &protocol).0;
}
