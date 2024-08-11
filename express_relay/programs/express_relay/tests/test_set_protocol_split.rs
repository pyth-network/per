pub mod helpers;

use anchor_lang::prelude::*;
use solana_program_test::{ProgramTest, tokio};
use solana_sdk::{account::Account, signature::Keypair, signer::Signer};
use express_relay::state::{ConfigProtocol, ExpressRelayMetadata};
use helpers::{initialize, set_protocol_split};

#[tokio::test]
async fn test_set_protocol_split() {
    let mut program_test = ProgramTest::new(
        "express_relay",
        express_relay::id(),
        None,
    );

    let payer = Keypair::new();
    let admin = Keypair::new();
    let relayer_signer = Keypair::new().pubkey();
    let fee_receiver_relayer = Keypair::new().pubkey();
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

    initialize(&mut program_context, &payer, admin.pubkey(), relayer_signer, fee_receiver_relayer, split_protocol_default, split_relayer).await;

    let protocol = Keypair::new().pubkey();
    let split_protocol = 1000;

    let (protocol_config_acc, express_relay_metadata_acc) = set_protocol_split(&mut program_context, admin, protocol, split_protocol).await;

    let express_relay_metadata_data = ExpressRelayMetadata::try_deserialize(&mut express_relay_metadata_acc.data.as_ref()).unwrap();
    assert_eq!(express_relay_metadata_data.split_protocol_default, split_protocol_default);

    let protocol_config_data = ConfigProtocol::try_deserialize(&mut protocol_config_acc.data.as_ref()).unwrap();
    assert_eq!(protocol_config_data.split, split_protocol);
}
