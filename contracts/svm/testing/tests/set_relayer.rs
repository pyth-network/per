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
            set_relayer::set_relayer_instruction,
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
fn test_set_relayer() {
    let setup_result = setup(None).expect("setup failed");

    let mut svm = setup_result.svm;
    let admin = setup_result.admin;

    let relayer_signer_new = Keypair::new().pubkey();
    let fee_receiver_relayer_new = Keypair::new().pubkey();
    let set_relayer_ix =
        set_relayer_instruction(&admin, relayer_signer_new, fee_receiver_relayer_new);
    submit_transaction(&mut svm, &[set_relayer_ix], &admin, &[&admin])
        .expect("Transaction failed unexpectedly");

    let express_relay_metadata = get_express_relay_metadata(&mut svm);

    assert_eq!(express_relay_metadata.relayer_signer, relayer_signer_new);
    assert_eq!(
        express_relay_metadata.fee_receiver_relayer,
        fee_receiver_relayer_new
    );
}

#[test]
fn test_set_relayer_fail_wrong_admin() {
    let setup_result = setup(None).expect("setup failed");

    let mut svm = setup_result.svm;
    let wrong_admin = generate_and_fund_key(&mut svm);

    let relayer_signer_new = Keypair::new().pubkey();
    let fee_receiver_relayer_new = Keypair::new().pubkey();
    let set_relayer_ix =
        set_relayer_instruction(&wrong_admin, relayer_signer_new, fee_receiver_relayer_new);
    let tx_result = submit_transaction(&mut svm, &[set_relayer_ix], &wrong_admin, &[&wrong_admin])
        .expect_err("Transaction should have failed");

    assert_custom_error(
        tx_result.err,
        0,
        InstructionError::Custom(AnchorErrorCode::ConstraintHasOne.into()),
    );
}
