pub mod helpers;

use anchor_lang::prelude::*;
use solana_program_test::{ProgramTest, tokio};
use solana_sdk::{account::Account, signature::Keypair, signer::Signer};
use express_relay::state::{FEE_SPLIT_PRECISION, SEED_EXPRESS_RELAY_FEES, SEED_METADATA};
use helpers::{initialize, permission};

#[tokio::test]
async fn test_permission() {
    let mut program_test = ProgramTest::new(
        "express_relay",
        express_relay::id(),
        None,
    );

    let payer = Keypair::new();
    let admin = Keypair::new();
    let relayer_signer = Keypair::new();
    let fee_receiver_relayer = Keypair::new().pubkey();
    let split_protocol_default: u64 = 2000;
    let split_relayer: u64 = 5000;

    let searcher = Keypair::new();

    let protocol = Keypair::new().pubkey();
    let fee_receiver_protocol = Pubkey::find_program_address(&[SEED_EXPRESS_RELAY_FEES], &protocol).0;

    program_test.add_account(
        payer.pubkey(),
        Account {
            lamports: 1_000_000_000,
            ..Account::default()
        },
    );
    program_test.add_account(
        searcher.pubkey(),
        Account {
            lamports: 1_000_000_000,
            ..Account::default()
        },
    );
    program_test.add_account(
        relayer_signer.pubkey(),
        Account {
            lamports: 1_000_000_000,
            ..Account::default()
        },
    );
    program_test.add_account(
        fee_receiver_protocol,
        Account {
            lamports: 1_000_000_000,
            ..Account::default()
        },
    );
    program_test.add_account(
        fee_receiver_relayer,
        Account {
            lamports: 1_000_000_000,
            ..Account::default()
        },
    );

    let mut program_context = program_test.start_with_context().await;

    initialize(&mut program_context, &payer, admin.pubkey(), relayer_signer.pubkey(), fee_receiver_relayer, split_protocol_default, split_relayer).await;

    let permission_key = Pubkey::find_program_address(&[b"permission_key"], &protocol).0;
    let bid_amount: u64 = 1000;

    let express_relay_metadata = Pubkey::find_program_address(&[SEED_METADATA], &express_relay::id()).0;
    let balance_express_relay_metadata_pre = program_context.banks_client.get_balance(express_relay_metadata).await.unwrap();
    let balance_fee_receiver_relayer_pre = program_context.banks_client.get_balance(fee_receiver_relayer).await.unwrap();
    let balance_fee_receiver_protocol_pre = program_context.banks_client.get_balance(fee_receiver_protocol).await.unwrap();
    let balance_searcher_pre = program_context.banks_client.get_balance(searcher.pubkey()).await.unwrap();

    let (tx_success, balance_express_relay_metadata_post, balance_fee_receiver_relayer_post, balance_fee_receiver_protocol_post, balance_searcher_post) = permission(
        &mut program_context,
        relayer_signer,
        searcher,
        protocol,
        fee_receiver_relayer,
        fee_receiver_protocol,
        permission_key,
        bid_amount
    ).await;

    // expected total transfer amount to protocol
    let expected_fee_protocol = bid_amount * split_protocol_default / FEE_SPLIT_PRECISION;

    // expected total transfer amount to relayer
    let expected_fee_relayer = bid_amount.saturating_sub(expected_fee_protocol) * split_relayer / FEE_SPLIT_PRECISION;

    // expected total transfer amount to express_relay
    let expected_fee_express_relay = bid_amount.saturating_sub(expected_fee_protocol).saturating_sub(expected_fee_relayer);

    assert!(tx_success);
    assert_eq!(balance_fee_receiver_protocol_post-balance_fee_receiver_protocol_pre, expected_fee_protocol);
    assert_eq!(balance_fee_receiver_relayer_post-balance_fee_receiver_relayer_pre, expected_fee_relayer);
    assert_eq!(balance_express_relay_metadata_post-balance_express_relay_metadata_pre, expected_fee_express_relay);
    assert_eq!(balance_searcher_pre-balance_searcher_post, bid_amount);
}
