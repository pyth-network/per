pub mod helpers;

use anchor_lang::prelude::*;
use solana_program::system_instruction::transfer;
use solana_program_test::{tokio, ProgramTest};
use solana_sdk::{account::Account, signature::Keypair, signer::Signer, transaction::Transaction};
use express_relay::state::{SEED_METADATA, RESERVE_EXPRESS_RELAY_METADATA};
use helpers::{initialize, withdraw_fees};

#[tokio::test]
async fn test_withdraw_fees() {
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
            lamports: 10_000_000_000,
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

    let express_relay_metadata = Pubkey::find_program_address(&[SEED_METADATA], &express_relay::id()).0;

    let transfer_ix = transfer(&payer.pubkey(), &express_relay_metadata, 9_000_000_000);
    let mut transfer_tx = Transaction::new_with_payer(&[transfer_ix], Some(&payer.pubkey()));
    let recent_blockhash = program_context.last_blockhash.clone();

    transfer_tx.partial_sign(&[&payer], recent_blockhash);
    program_context
        .banks_client
        .process_transaction(transfer_tx)
        .await
        .unwrap();


    initialize(&mut program_context, &payer, admin.pubkey(), relayer_signer, fee_receiver_relayer, split_protocol_default, split_relayer).await;

    let (balance_express_relay_metadata, balance_admin) = withdraw_fees(&mut program_context, admin).await;

    let rent_express_relay_metadata = Rent::default().minimum_balance(RESERVE_EXPRESS_RELAY_METADATA).max(1);

    assert_eq!(balance_express_relay_metadata, rent_express_relay_metadata);
    assert!(balance_admin > 9_000_000_000);
}
