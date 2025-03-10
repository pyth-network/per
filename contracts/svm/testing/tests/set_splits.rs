use {
    anchor_lang::error::ErrorCode as AnchorErrorCode,
    express_relay::{
        error::ErrorCode,
        state::FEE_SPLIT_PRECISION,
    },
    solana_sdk::instruction::InstructionError,
    testing::{
        express_relay::{
            helpers::get_express_relay_metadata,
            set_splits::set_splits_instruction,
        },
        helpers::{
            assert_custom_error,
            generate_and_fund_key,
            submit_transaction,
        },
        setup::setup,
    },
};

#[test]
fn test_set_splits() {
    let setup_result = setup(None).expect("setup failed");

    let mut svm = setup_result.svm;
    let admin = setup_result.admin;

    let split_router_default_new: u64 = 5000;
    let split_relayer_new: u64 = 1000;
    let set_splits_ix = set_splits_instruction(&admin, split_router_default_new, split_relayer_new);
    submit_transaction(&mut svm, &[set_splits_ix], &admin, &[&admin])
        .expect("Transaction failed unexpectedly");

    let express_relay_metadata = get_express_relay_metadata(&mut svm);

    assert_eq!(
        express_relay_metadata.split_router_default,
        split_router_default_new
    );
    assert_eq!(express_relay_metadata.split_relayer, split_relayer_new);
}

#[test]
fn test_set_splits_fail_wrong_admin() {
    let setup_result = setup(None).expect("setup failed");

    let mut svm = setup_result.svm;
    let wrong_admin = generate_and_fund_key(&mut svm);

    let split_router_default_new: u64 = 5000;
    let split_relayer_new: u64 = 1000;
    let set_splits_ix =
        set_splits_instruction(&wrong_admin, split_router_default_new, split_relayer_new);
    let tx_result = submit_transaction(&mut svm, &[set_splits_ix], &wrong_admin, &[&wrong_admin])
        .expect_err("Transaction should have failed");

    assert_custom_error(
        tx_result.err,
        0,
        InstructionError::Custom(AnchorErrorCode::ConstraintHasOne.into()),
    );
}

#[test]
fn test_set_splits_fail_high_split_router() {
    let setup_result = setup(None).expect("setup failed");

    let mut svm = setup_result.svm;
    let admin = setup_result.admin;

    let split_router_default_new: u64 = FEE_SPLIT_PRECISION + 1;
    let split_relayer_new: u64 = 1000;
    let set_splits_ix = set_splits_instruction(&admin, split_router_default_new, split_relayer_new);
    let tx_result = submit_transaction(&mut svm, &[set_splits_ix], &admin, &[&admin])
        .expect_err("Transaction should have failed");

    assert_custom_error(
        tx_result.err,
        0,
        InstructionError::Custom(ErrorCode::FeeSplitLargerThanPrecision.into()),
    );
}

#[test]
fn test_set_splits_fail_high_split_relayer() {
    let setup_result = setup(None).expect("setup failed");

    let mut svm = setup_result.svm;
    let admin = setup_result.admin;

    let split_router_default_new: u64 = 5000;
    let split_relayer_new: u64 = FEE_SPLIT_PRECISION + 1;
    let set_splits_ix = set_splits_instruction(&admin, split_router_default_new, split_relayer_new);
    let tx_result = submit_transaction(&mut svm, &[set_splits_ix], &admin, &[&admin])
        .expect_err("Transaction should have failed");

    assert_custom_error(
        tx_result.err,
        0,
        InstructionError::Custom(ErrorCode::FeeSplitLargerThanPrecision.into()),
    );
}
