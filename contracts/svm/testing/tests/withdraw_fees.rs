use {
    anchor_lang::error::ErrorCode as AnchorErrorCode,
    express_relay::state::RESERVE_EXPRESS_RELAY_METADATA,
    solana_sdk::{
        native_token::LAMPORTS_PER_SOL,
        signature::Keypair,
        signer::Signer,
    },
    testing::{
        express_relay::{
            helpers::get_express_relay_metadata_key,
            withdraw_fees::withdraw_fees_instruction,
        },
        helpers::{
            assert_custom_error,
            generate_and_fund_key,
            get_balance,
            submit_transaction,
        },
        setup::{
            setup,
            SetupParams,
        },
    },
};

#[test]
fn test_withdraw_fees() {
        let setup_result = setup(None)
    .expect("setup failed");

    let mut svm = setup_result.svm;
    let admin = setup_result.admin;

    let fee_receiver_admin = Keypair::new();
    let withdraw_fees_ix = withdraw_fees_instruction(&admin, fee_receiver_admin.pubkey());
    let express_relay_metadata_key = get_express_relay_metadata_key();
    let total_fees: u64 = LAMPORTS_PER_SOL;
    svm.airdrop(&express_relay_metadata_key, total_fees)
        .unwrap();

    let balance_express_relay_metadata_pre = get_balance(&svm, &express_relay_metadata_key);
    let balance_fee_receiver_admin_pre = get_balance(&svm, &fee_receiver_admin.pubkey());

    submit_transaction(&mut svm, &[withdraw_fees_ix], &admin, &[&admin])
        .expect("Transaction failed unexpectedly");

    let balance_express_relay_metadata_post = get_balance(&svm, &express_relay_metadata_key);
    let balance_fee_receiver_admin_post = get_balance(&svm, &fee_receiver_admin.pubkey());

    assert_eq!(
        balance_express_relay_metadata_pre - balance_express_relay_metadata_post,
        total_fees
    );
    assert_eq!(
        balance_fee_receiver_admin_post - balance_fee_receiver_admin_pre,
        total_fees
    );
    assert_eq!(
        balance_express_relay_metadata_post,
        svm.minimum_balance_for_rent_exemption(RESERVE_EXPRESS_RELAY_METADATA)
    );
}

#[test]
fn test_withdraw_fees_fail_wrong_admin() {
        let setup_result = setup(None)
    .expect("setup failed");

    let mut svm = setup_result.svm;
    let wrong_admin = generate_and_fund_key(&mut svm);

    let fee_receiver_admin = Keypair::new();
    let withdraw_fees_ix = withdraw_fees_instruction(&wrong_admin, fee_receiver_admin.pubkey());
    let tx_result =
        submit_transaction(&mut svm, &[withdraw_fees_ix], &wrong_admin, &[&wrong_admin])
            .expect_err("Transaction should have failed");

    assert_custom_error(tx_result.err, 0, AnchorErrorCode::ConstraintHasOne.into());
}
