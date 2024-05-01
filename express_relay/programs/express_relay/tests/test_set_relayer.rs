pub mod helpers;

use anchor_lang::prelude::*;
use solana_program_test::{ProgramTest, tokio};
use solana_sdk::{signature::Keypair, signer::Signer};
use express_relay::{state::ExpressRelayMetadata};
use helpers::{initialize, set_relayer};

#[tokio::test]
async fn test_set_relayer() {
    let program_test = ProgramTest::new(
        "express_relay",
        express_relay::id(),
        None,
    );

    let admin = Keypair::new();
    let relayer_signer = Keypair::new().pubkey();
    let relayer_fee_receiver = Keypair::new().pubkey();
    let split_protocol_default: u64 = 200_000_000_000_000_000;
    let split_relayer: u64 = 50_000_000_000_000_000;

    let express_relay_metadata_acc = initialize(program_test, admin.pubkey(), relayer_signer, relayer_fee_receiver, split_protocol_default, split_relayer).await;

    let express_relay_metadata_data = ExpressRelayMetadata::try_deserialize(&mut express_relay_metadata_acc.data.as_ref()).unwrap();
    assert_eq!(express_relay_metadata_data.relayer_signer, relayer_signer);
    assert_eq!(express_relay_metadata_data.relayer_fee_receiver, relayer_fee_receiver);

    let new_relayer_signer = Keypair::new().pubkey();
    let new_relayer_fee_receiver = Keypair::new().pubkey();

    // TODO: fix move issue
    // let express_relay_metadata_acc_2 = set_relayer(program_test, admin, new_relayer_signer, new_relayer_fee_receiver).await;

    // let express_relay_metadata_data_2 = ExpressRelayMetadata::try_deserialize(&mut express_relay_metadata_acc_2.data.as_ref()).unwrap();
    // assert_eq!(express_relay_metadata_data_2.relayer_signer, new_relayer_signer);
    // assert_eq!(express_relay_metadata_data_2.relayer_fee_receiver, new_relayer_fee_receiver);
}
