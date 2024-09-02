use {
    anchor_lang::error::ErrorCode as AnchorErrorCode,
    express_relay::{
        error::ErrorCode,
        state::FEE_SPLIT_PRECISION,
    },
    solana_sdk::{
        signature::Keypair,
        signer::Signer,
    },
    testing::{
        express_relay::{
            helpers::get_router_config,
            set_router_split::set_router_split_instruction,
        },
        helpers::{
            assert_custom_error,
            generate_and_fund_key,
            submit_transaction,
        },
        setup::{
            setup,
            SetupParams,
        },
    },
};

#[test]
fn test_set_router_split() {
    let setup_result = setup(SetupParams {
        split_router_default: 4000,
        split_relayer:        2000,
    })
    .expect("setup failed");

    let mut svm = setup_result.svm;
    let admin = setup_result.admin;

    let router = Keypair::new().pubkey();
    let split_router: u64 = 5000;
    let set_router_split_ix = set_router_split_instruction(&admin, router, split_router);
    submit_transaction(&mut svm, &[set_router_split_ix], &admin, &[&admin])
        .expect("Transaction failed unexpectedly");

    let router_config = get_router_config(svm, router).expect("Router Config not initialized");

    assert_eq!(router_config.router, router);
    assert_eq!(router_config.split, split_router);
}

#[test]
fn test_set_router_split_fail_wrong_admin() {
    let setup_result = setup(SetupParams {
        split_router_default: 4000,
        split_relayer:        2000,
    })
    .expect("setup failed");

    let mut svm = setup_result.svm;
    let wrong_admin = generate_and_fund_key(&mut svm);

    let router = Keypair::new().pubkey();
    let split_router: u64 = 5000;
    let set_router_split_ix = set_router_split_instruction(&wrong_admin, router, split_router);
    let tx_result = submit_transaction(
        &mut svm,
        &[set_router_split_ix],
        &wrong_admin,
        &[&wrong_admin],
    )
    .expect_err("Transaction should have failed");

    assert_custom_error(tx_result.err, 0, AnchorErrorCode::ConstraintHasOne.into());
}

#[test]
fn test_set_router_split_fail_high_split() {
    let setup_result = setup(SetupParams {
        split_router_default: 4000,
        split_relayer:        2000,
    })
    .expect("setup failed");

    let mut svm = setup_result.svm;
    let admin = setup_result.admin;

    let router = Keypair::new().pubkey();
    let split_router: u64 = FEE_SPLIT_PRECISION + 1;
    let set_router_split_ix = set_router_split_instruction(&admin, router, split_router);
    let tx_result = submit_transaction(&mut svm, &[set_router_split_ix], &admin, &[&admin])
        .expect_err("Transaction should have failed");

    assert_custom_error(
        tx_result.err,
        0,
        ErrorCode::FeeSplitLargerThanPrecision.into(),
    );
}
