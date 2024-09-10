pub mod helpers;

use {
    anchor_lang::AccountDeserialize,
    dummy::{
        FeesCount,
        SEED_FEES_COUNT,
    },
    express_relay::{
        error::ErrorCode as ExpressRelayErrorCode,
        sdk::test_helpers::add_express_relay_submit_bid_instruction,
    },
    helpers::{
        helpers_express_relay::{
            create_count_fees_ix,
            create_do_nothing_ix,
            setup,
        },
        helpers_general::{
            assert_custom_error,
            create_and_submit_tx,
        },
    },
    solana_program_test::tokio,
    solana_sdk::{
        instruction::Instruction,
        pubkey::Pubkey,
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

    let ix_do_nothing = create_do_nothing_ix(setup_info.payer.pubkey(), permission, router);
    let ixs: [Instruction; 2] = add_express_relay_submit_bid_instruction(
        &mut [ix_do_nothing].to_vec(),
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

    let ix_do_nothing = create_do_nothing_ix(setup_info.payer.pubkey(), permission, router_real);
    let ixs: [Instruction; 2] = add_express_relay_submit_bid_instruction(
        &mut [ix_do_nothing].to_vec(),
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

#[tokio::test]
async fn test_check_fees() {
    let bid_amount = 20;

    let router = Keypair::new().pubkey();
    let setup_info = setup(router).await;
    let mut program_test_context = setup_info.program_test_context;

    let permission = Keypair::new().pubkey();

    let ix_count_fees = create_count_fees_ix(setup_info.payer.pubkey(), permission, router);
    let ixs: [Instruction; 2] = add_express_relay_submit_bid_instruction(
        &mut [ix_count_fees].to_vec(),
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

    let fees_count = Pubkey::find_program_address(&[SEED_FEES_COUNT], &dummy::id()).0;
    let account_fees_count = program_test_context
        .banks_client
        .get_account(fees_count)
        .await
        .unwrap()
        .unwrap();
    let data_fees_count =
        FeesCount::try_deserialize(&mut account_fees_count.data.as_ref()).unwrap();

    assert_eq!(balance_router_post, balance_router_pre + bid_amount);
    assert_eq!(data_fees_count.count, bid_amount);
}
