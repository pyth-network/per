use {
    anchor_lang::error::ErrorCode as AnchorErrorCode,
    solana_sdk::{
        instruction::InstructionError,
        pubkey::Pubkey,
    },
    testing::{
        express_relay::{
            helpers::get_express_relay_metadata,
            set_secondary_relayer::set_secondary_relayer_instruction,
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
fn test_set_secondary_relayer() {
    let setup_result = setup(None).expect("setup failed");

    let mut svm = setup_result.svm;
    let admin = setup_result.admin;

    let primary_relayer_signer = get_express_relay_metadata(&mut svm).relayer_signer;

    let secondary_relayer_signer_new = Pubkey::new_unique();
    let set_secondary_relayer_ix =
        set_secondary_relayer_instruction(&admin, secondary_relayer_signer_new);
    submit_transaction(&mut svm, &[set_secondary_relayer_ix], &admin, &[&admin])
        .expect("Transaction failed unexpectedly");

    let express_relay_metadata = get_express_relay_metadata(&mut svm);

    assert_eq!(
        express_relay_metadata.secondary_relayer_signer,
        secondary_relayer_signer_new
    );
    assert_eq!(
        express_relay_metadata.relayer_signer,
        primary_relayer_signer
    );
}

#[test]
fn test_set_secondary_relayer_fail_wrong_admin() {
    let setup_result = setup(None).expect("setup failed");

    let mut svm = setup_result.svm;
    let wrong_admin = generate_and_fund_key(&mut svm);

    let relayer_signer_new = Pubkey::new_unique();
    let set_secondary_relayer_ix =
        set_secondary_relayer_instruction(&wrong_admin, relayer_signer_new);
    let tx_result = submit_transaction(
        &mut svm,
        &[set_secondary_relayer_ix],
        &wrong_admin,
        &[&wrong_admin],
    )
    .expect_err("Transaction should have failed");

    assert_custom_error(
        tx_result.err,
        0,
        InstructionError::Custom(AnchorErrorCode::ConstraintHasOne.into()),
    );
}
