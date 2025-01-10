use {
    anchor_lang::error::ErrorCode as AnchorErrorCode,
    express_relay::{
        error::ErrorCode,
        state::FEE_SPLIT_PRECISION,
    },
    litesvm::LiteSVM,
    testing::{
        express_relay::{
            helpers::get_express_relay_metadata,
            set_swap_platform_fee::set_swap_platform_fee_instruction,
        },
        helpers::{
            assert_custom_error,
            generate_and_fund_key,
            submit_transaction,
        },
        setup::{
            setup,
            SetupParams,
            SetupResult,
        },
    },
};

fn assert_swap_platform_fee(svm: &mut LiteSVM, expected_fee: u64) {
    let express_relay_metadata = get_express_relay_metadata(svm);
    assert_eq!(express_relay_metadata.swap_platform_fee_bps, expected_fee);
}

#[test]
fn test_set_swap_platform_fee() {
    let SetupResult { mut svm, admin, .. } = setup(SetupParams {
        split_router_default: 4000,
        split_relayer:        2000,
    })
    .expect("setup failed");

    assert_swap_platform_fee(&mut svm, 0);

    let set_swap_platform_fee_ix = set_swap_platform_fee_instruction(&admin, 1000);
    submit_transaction(&mut svm, &[set_swap_platform_fee_ix], &admin, &[&admin])
        .expect("Transaction failed unexpectedly");

    assert_swap_platform_fee(&mut svm, 1000);

    let set_swap_platform_fee_ix = set_swap_platform_fee_instruction(&admin, 2000);
    submit_transaction(&mut svm, &[set_swap_platform_fee_ix], &admin, &[&admin])
        .expect("Transaction failed unexpectedly");

    assert_swap_platform_fee(&mut svm, 2000);
}


#[test]
fn test_set_swap_platform_fee_wrong_admin() {
    let SetupResult { mut svm, .. } = setup(SetupParams {
        split_router_default: 4000,
        split_relayer:        2000,
    })
    .expect("setup failed");

    let wrong_admin = generate_and_fund_key(&mut svm);

    let set_swap_platform_fee_ix = set_swap_platform_fee_instruction(&wrong_admin, 1000);

    let tx_result = submit_transaction(
        &mut svm,
        &[set_swap_platform_fee_ix],
        &wrong_admin,
        &[&wrong_admin],
    )
    .expect_err("Transaction should have failed");

    assert_custom_error(tx_result.err, 0, AnchorErrorCode::ConstraintHasOne.into());
}

#[test]
fn test_set_swap_platform_fee_fail_high_split_router() {
    let SetupResult { mut svm, admin, .. } = setup(SetupParams {
        split_router_default: 4000,
        split_relayer:        2000,
    })
    .expect("setup failed");

    let set_swap_platform_fee_ix =
        set_swap_platform_fee_instruction(&admin, FEE_SPLIT_PRECISION + 1);
    let tx_result = submit_transaction(&mut svm, &[set_swap_platform_fee_ix], &admin, &[&admin])
        .expect_err("Transaction should have failed");

    assert_custom_error(
        tx_result.err,
        0,
        ErrorCode::FeeSplitLargerThanPrecision.into(),
    );
}
