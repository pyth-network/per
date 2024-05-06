pub mod helpers;

use anchor_lang::prelude::*;
use solana_program_test::{ProgramTest, tokio};
use solana_sdk::{account::Account, signature::Keypair, signer::Signer, system_instruction};
use express_relay::{state::SEED_PERMISSION};
use helpers::{initialize, express_relay_tx};

#[tokio::test]
async fn test_permission_depermission() {
    let mut program_test = ProgramTest::new(
        "express_relay",
        express_relay::id(),
        None,
    );

    let payer = Keypair::new();
    let searcher = Keypair::new();
    let admin = Keypair::new().pubkey();
    let relayer_signer = Keypair::new();
    let relayer_fee_receiver = Keypair::new().pubkey();
    let split_protocol_default: u64 = 2000;
    let split_relayer: u64 = 1000;

    let searcher_lamports = 2_000_000_000;

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
            lamports: searcher_lamports,
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

    initialize(&mut program_context, payer, admin, relayer_signer.pubkey(), relayer_fee_receiver, split_protocol_default, split_relayer).await;

    let permission_id: [u8; 8] = [0; 8];
    let bid_id: [u8; 16] = [0; 16];
    // NOTE: bid needs to be large enough to avoid running into insufficient rent errors
    let bid_amount = 100_000_000;
    // TODO: replace with another program's id?
    let protocol = express_relay::id();
    let permission = Pubkey::find_program_address(&[SEED_PERMISSION, &protocol.to_bytes(), &permission_id], &express_relay::id()).0;

    let send_sol_ix = system_instruction::transfer(&searcher.pubkey(), &permission, bid_amount);

    let (permission_balance, relayer_fee_receiver_balance, protocol_balance) = express_relay_tx(
        &mut program_context,
        searcher,
        relayer_signer,
        protocol,
        permission_id.into(),
        bid_id,
        bid_amount,
        send_sol_ix
    ).await;

    let expected_fees_protocol = 20_000_000;
    let expected_fees_relayer = 8_000_000;
    assert_eq!(permission_balance, 0);
    assert_eq!(protocol_balance, expected_fees_protocol);
    assert_eq!(relayer_fee_receiver_balance, expected_fees_relayer);
}
