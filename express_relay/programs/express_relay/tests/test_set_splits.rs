pub mod helpers;

use anchor_lang::prelude::*;
use solana_program_test::{ProgramTest, tokio};
use solana_sdk::{account::Account, signature::Keypair, signer::Signer};
use express_relay::{state::ExpressRelayMetadata};
use helpers::{initialize, set_splits};

#[tokio::test]
async fn test_set_splits() {
    let mut program_test = ProgramTest::new(
        "express_relay",
        express_relay::id(),
        None,
    );

    let payer = Keypair::new();
    let admin = Keypair::new();
    let relayer_signer = Keypair::new().pubkey();
    let relayer_fee_receiver = Keypair::new().pubkey();
    let split_protocol_default: u64 = 2000;
    let split_relayer: u64 = 5000;

    program_test.add_account(
        payer.pubkey(),
        Account {
            lamports: 1_000_000_000,
            ..Account::default()
        },
    );
    program_test.add_account(
        admin.pubkey(),
        Account {
            lamports: 1_000_000_000,
            ..Account::default()
        },
    );

    let mut program_context = program_test.start_with_context().await;

    initialize(&mut program_context, payer, admin.pubkey(), relayer_signer, relayer_fee_receiver, split_protocol_default, split_relayer).await;

    let new_split_protocol_default: u64 = 1000;
    let new_split_relayer: u64 = 3000;

    let express_relay_metadata_acc_2 = set_splits(&mut program_context, admin, new_split_protocol_default, new_split_relayer).await;

    let express_relay_metadata_data_2 = ExpressRelayMetadata::try_deserialize(&mut express_relay_metadata_acc_2.data.as_ref()).unwrap();
    assert_eq!(express_relay_metadata_data_2.split_protocol_default, new_split_protocol_default);
    assert_eq!(express_relay_metadata_data_2.split_relayer, new_split_relayer);
}
