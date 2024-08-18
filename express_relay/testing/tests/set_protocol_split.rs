use express_relay::{error::ErrorCode, state::FEE_SPLIT_PRECISION};
use solana_sdk::{signature::Keypair, signer::Signer};
use anchor_lang::error::ErrorCode as AnchorErrorCode;
use testing::{express_relay::{helpers::get_protocol_config, set_protocol_split::get_set_protocol_split_instruction}, helpers::{assert_custom_error, generate_and_fund_key, submit_transaction}, setup::{setup, SetupParams}};

#[test]
fn test_set_protocol_split() {
    let setup_result = setup(SetupParams {
        split_protocol_default: 4000,
        split_relayer: 2000,
    }).expect("setup failed");

    let mut svm = setup_result.svm;
    let admin = setup_result.admin;

    let protocol = Keypair::new().pubkey();
    let split_protocol: u64 = 5000;
    let set_protocol_split_ix = get_set_protocol_split_instruction(&admin, protocol, split_protocol);
    submit_transaction(&mut svm, &[set_protocol_split_ix], &admin, &[&admin]).expect("Transaction failed unexpectedly");

    let protocol_config = get_protocol_config(svm, protocol).expect("Protocol Config not initialized");

    assert_eq!(protocol_config.protocol, protocol);
    assert_eq!(protocol_config.split, split_protocol);
}

#[test]
fn test_set_protocol_split_fail_wrong_admin() {
    let setup_result = setup(SetupParams {
        split_protocol_default: 4000,
        split_relayer: 2000,
    }).expect("setup failed");

    let mut svm = setup_result.svm;
    let wrong_admin = generate_and_fund_key(&mut svm);

    let protocol = Keypair::new().pubkey();
    let split_protocol: u64 = 5000;
    let set_protocol_split_ix = get_set_protocol_split_instruction(&wrong_admin, protocol, split_protocol);
    let tx_result = submit_transaction(&mut svm, &[set_protocol_split_ix], &wrong_admin, &[&wrong_admin]).expect_err("Transaction should have failed");

    assert_custom_error(tx_result.err, 0, AnchorErrorCode::ConstraintHasOne.into());
}

#[test]
fn test_set_protocol_split_fail_high_split() {
    let setup_result = setup(SetupParams {
        split_protocol_default: 4000,
        split_relayer: 2000,
    }).expect("setup failed");

    let mut svm = setup_result.svm;
    let admin = setup_result.admin;

    let protocol = Keypair::new().pubkey();
    let split_protocol: u64 = FEE_SPLIT_PRECISION+1;
    let set_protocol_split_ix = get_set_protocol_split_instruction(&admin, protocol, split_protocol);
    let tx_result = submit_transaction(&mut svm, &[set_protocol_split_ix], &admin, &[&admin]).expect_err("Transaction should have failed");

    assert_custom_error(tx_result.err, 0, ErrorCode::FeeSplitLargerThanPrecision.into());
}
