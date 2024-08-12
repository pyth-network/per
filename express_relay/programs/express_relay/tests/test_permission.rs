pub mod helpers;

use anchor_lang::prelude::*;
use solana_program_test::{ProgramTest, tokio};
use solana_sdk::{account::Account, signature::Keypair, signer::Signer};
use express_relay::state::{FEE_SPLIT_PRECISION, RESERVE_EXPRESS_RELAY_METADATA, SEED_EXPRESS_RELAY_FEES};
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

    let mut program_context = program_test.start_with_context().await;

    initialize(&mut program_context, &payer, admin.pubkey(), relayer_signer.pubkey(), fee_receiver_relayer, split_protocol_default, split_relayer).await;

    let protocol = Keypair::new().pubkey();
    let permission_id: [u8; 32] = [0; 32];
    let fee_receiver_protocol = Pubkey::find_program_address(&[SEED_EXPRESS_RELAY_FEES], &protocol).0;
    let bid_amount: u64 = 1000;

    let express_relay_metadata = Pubkey::find_program_address(&[SEED_EXPRESS_RELAY_FEES], &express_relay::id()).0;
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
        permission_id,
        bid_amount
    ).await;

    // expected total transfer amount to protocol
    let expected_fee_protocol = bid_amount * split_protocol_default / FEE_SPLIT_PRECISION;
    let expected_amount_protocol: u64;
    if balance_fee_receiver_protocol_pre == 0 {
        let rent_fee_receiver_protocol = Rent::default().minimum_balance(0).max(1);
        expected_amount_protocol = expected_fee_protocol + rent_fee_receiver_protocol;
    } else {
        expected_amount_protocol = expected_fee_protocol;
    }

    // expected total transfer amount to relayer
    let expected_fee_relayer = bid_amount.saturating_sub(expected_fee_protocol) * split_relayer / FEE_SPLIT_PRECISION;
    let expected_amount_relayer: u64;
    if balance_fee_receiver_relayer_pre == 0 {
        let rent_fee_receiver_relayer = Rent::default().minimum_balance(0).max(1);
        expected_amount_relayer = expected_fee_relayer + rent_fee_receiver_relayer;
    } else {
        expected_amount_relayer = expected_fee_relayer;
    }

    // expected total transfer amount to express_relay
    let expected_fee_express_relay = bid_amount.saturating_sub(expected_fee_protocol).saturating_sub(expected_fee_relayer);
    let expected_amount_express_relay_metadata: u64;
    if balance_express_relay_metadata_pre == 0 {
        let rent_express_relay_metadata = Rent::default().minimum_balance(RESERVE_EXPRESS_RELAY_METADATA).max(1);
        expected_amount_express_relay_metadata = expected_fee_express_relay + rent_express_relay_metadata;
    } else {
        expected_amount_express_relay_metadata = expected_fee_express_relay;
    }

    assert!(tx_success);
    assert_eq!(balance_fee_receiver_protocol_post-balance_fee_receiver_protocol_pre, expected_amount_protocol);
    assert_eq!(balance_fee_receiver_relayer_post-balance_fee_receiver_relayer_pre, expected_amount_relayer);
    assert_eq!(balance_express_relay_metadata_post-balance_express_relay_metadata_pre, expected_amount_express_relay_metadata);
    // searcher pays for rent of relayer fee receiver and protocol fee receiver, but not for express_relay_metadata
    assert_eq!(balance_searcher_pre-balance_searcher_post, expected_amount_protocol+expected_amount_relayer+expected_fee_express_relay);
}
