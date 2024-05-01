pub mod helpers;

use anchor_lang::prelude::*;
use solana_program_test::{ProgramTest, tokio};
use solana_sdk::{signature::Keypair, signer::Signer};
use express_relay::{state::ExpressRelayMetadata};
use helpers::initialize;

#[tokio::test]
async fn test_initialize() {
    let program_test = ProgramTest::new(
        "express_relay",
        express_relay::id(),
        None,
    );

    let admin = Keypair::new().pubkey();
    let relayer_signer = Keypair::new().pubkey();
    let relayer_fee_receiver = Keypair::new().pubkey();
    let split_protocol_default: u64 = 200_000_000_000_000_000;
    let split_relayer: u64 = 50_000_000_000_000_000;

    let express_relay_metadata_acc = initialize(program_test, admin, relayer_signer, relayer_fee_receiver, split_protocol_default, split_relayer).await;

    let express_relay_metadata_data = ExpressRelayMetadata::try_deserialize(&mut express_relay_metadata_acc.data.as_ref()).unwrap();
    assert_eq!(express_relay_metadata_data.admin, admin);
    assert_eq!(express_relay_metadata_data.relayer_signer, relayer_signer);
    assert_eq!(express_relay_metadata_data.relayer_fee_receiver, relayer_fee_receiver);
    assert_eq!(express_relay_metadata_data.split_protocol_default, split_protocol_default);
    assert_eq!(express_relay_metadata_data.split_relayer, split_relayer);
}
