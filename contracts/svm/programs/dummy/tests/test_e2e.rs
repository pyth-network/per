pub mod helpers;

use {
    express_relay::{
        error::ErrorCode as ExpressRelayErrorCode,
        sdk::test_helpers::add_express_relay_submit_bid_instruction,
    },
    helpers::{
        assert_custom_error,
        create_and_submit_tx,
        create_do_nothing_ix,
        setup,
    },
    solana_program_test::tokio,
    solana_sdk::{
        instruction::Instruction,
        signature::Keypair,
        signer::Signer,
    },
};

#[tokio::test]
async fn test_dummy_e2e() {
    let bid_amount = 1;

    let router = Keypair::new().pubkey();
    let setup_info = setup(router).await;
    let mut program_test_context = setup_info.program_test_context;

    let permission = Keypair::new().pubkey();

    let dummy_ix = create_do_nothing_ix(setup_info.payer.pubkey(), permission, router);
    let ixs: [Instruction; 2] = add_express_relay_submit_bid_instruction(
        &mut [dummy_ix].to_vec(),
        setup_info.payer.pubkey(),
        setup_info.relayer_signer.pubkey(),
        setup_info.fee_receiver_relayer.pubkey(),
        permission,
        router,
        bid_amount,
    )
    .try_into()
    .unwrap();

    let balance_router_pre = program_test_context
        .banks_client
        .get_balance(router)
        .await
        .unwrap();
    create_and_submit_tx(
        &mut program_test_context,
        &ixs,
        &setup_info.payer,
        &[&setup_info.payer, &setup_info.relayer_signer],
    )
    .await
    .unwrap();
    let balance_router_post = program_test_context
        .banks_client
        .get_balance(router)
        .await
        .unwrap();

    assert_eq!(balance_router_post, balance_router_pre + bid_amount);
}

#[tokio::test]
async fn test_dummy_e2e_fail_router_underfunded() {
    let bid_amount = 1;

    let router_fake = Keypair::new().pubkey();
    let router_real = Keypair::new().pubkey();
    let setup_info = setup(router_fake).await;
    let mut program_test_context = setup_info.program_test_context;

    let permission = Keypair::new().pubkey();

    let dummy_ix = create_do_nothing_ix(setup_info.payer.pubkey(), permission, router_real);
    let ixs: [Instruction; 2] = add_express_relay_submit_bid_instruction(
        &mut [dummy_ix].to_vec(),
        setup_info.payer.pubkey(),
        setup_info.relayer_signer.pubkey(),
        setup_info.fee_receiver_relayer.pubkey(),
        permission,
        router_real,
        bid_amount,
    )
    .try_into()
    .unwrap();

    let err = create_and_submit_tx(
        &mut program_test_context,
        &ixs,
        &setup_info.payer,
        &[&setup_info.payer, &setup_info.relayer_signer],
    )
    .await
    .expect_err("Transaction should have failed");
    assert_custom_error(
        err.unwrap(),
        1,
        ExpressRelayErrorCode::InsufficientRent.into(),
    );
}
