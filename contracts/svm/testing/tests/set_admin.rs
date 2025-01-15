use {
    anchor_lang::error::ErrorCode as AnchorErrorCode,
    solana_sdk::{
        instruction::InstructionError,
        signature::Keypair,
        signer::Signer,
    },
    testing::{
        express_relay::{
            helpers::get_express_relay_metadata,
            set_admin::set_admin_instruction,
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
fn test_set_admin() {
    let setup_result = setup(None).expect("setup failed");

    let mut svm = setup_result.svm;
    let admin = setup_result.admin;

    let admin_new = Keypair::new();
    let set_admin_ix = set_admin_instruction(&admin, admin_new.pubkey());
    submit_transaction(&mut svm, &[set_admin_ix], &admin, &[&admin])
        .expect("Transaction failed unexpectedly");

    let express_relay_metadata = get_express_relay_metadata(&mut svm);

    assert_eq!(express_relay_metadata.admin, admin_new.pubkey());
}

#[test]
fn test_set_admin_fail_wrong_admin() {
    let setup_result = setup(None).expect("setup failed");

    let mut svm = setup_result.svm;
    let wrong_admin = generate_and_fund_key(&mut svm);

    let admin_new = Keypair::new();
    let set_admin_ix = set_admin_instruction(&wrong_admin, admin_new.pubkey());
    let tx_result = submit_transaction(&mut svm, &[set_admin_ix], &wrong_admin, &[&wrong_admin])
        .expect_err("Transaction should have failed");

    assert_custom_error(
        tx_result.err,
        0,
        InstructionError::Custom(AnchorErrorCode::ConstraintHasOne.into()),
    );
}
