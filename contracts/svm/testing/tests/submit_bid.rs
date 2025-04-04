use {
    anchor_lang::error::ErrorCode as AnchorErrorCode,
    dummy::RESERVE_ACCOUNTING,
    express_relay::{
        error::ErrorCode,
        state::FEE_SPLIT_PRECISION,
    },
    solana_sdk::{
        instruction::{
            Instruction,
            InstructionError,
        },
        native_token::LAMPORTS_PER_SOL,
        pubkey::Pubkey,
        rent::Rent,
        signature::Keypair,
        signer::Signer,
        system_instruction::transfer,
    },
    testing::{
        dummy::do_nothing::do_nothing_instruction,
        express_relay::{
            helpers::{
                get_express_relay_metadata,
                get_express_relay_metadata_key,
            },
            submit_bid::bid_instructions,
        },
        helpers::{
            assert_custom_error,
            get_balance,
            submit_transaction,
            warp_to_unix,
            TX_FEE,
        },
        setup::setup,
    },
};

pub struct BidSetupResult {
    pub svm:                      litesvm::LiteSVM,
    pub relayer_signer:           Keypair,
    pub secondary_relayer_signer: Keypair,
    pub searcher:                 Keypair,
    pub fee_receiver_relayer:     Keypair,
    pub router:                   Pubkey,
    pub permission_key:           Pubkey,
    pub bid_amount:               u64,
    pub deadline:                 i64,
    pub ixs:                      Vec<Instruction>,
}

fn setup_bid() -> BidSetupResult {
    let setup_result = setup(None).expect("setup failed");

    let svm = setup_result.svm;
    let relayer_signer = setup_result.relayer_signer;
    let secondary_relayer_signer = setup_result.secondary_relayer_signer;
    let searcher = setup_result.searcher;
    let fee_receiver_relayer = setup_result.fee_receiver_relayer;
    let permission_key = Keypair::new().pubkey();
    let router = Keypair::new().pubkey();
    let bid_amount = LAMPORTS_PER_SOL;
    let deadline: i64 = 100_000_000_000;
    let ixs = [do_nothing_instruction(&searcher, permission_key, router)];

    BidSetupResult {
        svm,
        relayer_signer,
        secondary_relayer_signer,
        searcher,
        fee_receiver_relayer,
        router,
        permission_key,
        bid_amount,
        deadline,
        ixs: ixs.to_vec(),
    }
}

#[test]
fn test_bid() {
    let BidSetupResult {
        mut svm,
        relayer_signer,
        searcher,
        fee_receiver_relayer,
        router,
        permission_key,
        bid_amount,
        deadline,
        ixs,
        ..
    } = setup_bid();

    let bid_ixs = bid_instructions(
        &relayer_signer,
        &searcher,
        router,
        fee_receiver_relayer.pubkey(),
        permission_key,
        bid_amount,
        deadline,
        &ixs,
    );

    let express_relay_metadata_key = get_express_relay_metadata_key();

    let balance_router_pre = get_balance(&svm, &router);
    let balance_fee_receiver_relayer_pre = get_balance(&svm, &fee_receiver_relayer.pubkey());
    let balance_express_relay_metadata_pre = get_balance(&svm, &express_relay_metadata_key);
    let balance_searcher_pre = get_balance(&svm, &searcher.pubkey());

    submit_transaction(&mut svm, &bid_ixs, &searcher, &[&searcher, &relayer_signer])
        .expect("Transaction failed unexpectedly");

    let balance_router_post = get_balance(&svm, &router);
    let balance_fee_receiver_relayer_post = get_balance(&svm, &fee_receiver_relayer.pubkey());
    let balance_express_relay_metadata_post = get_balance(&svm, &express_relay_metadata_key);
    let balance_searcher_post = get_balance(&svm, &searcher.pubkey());

    let express_relay_metadata_acc = get_express_relay_metadata(&mut svm);
    let expected_fee_router =
        bid_amount * express_relay_metadata_acc.split_router_default / FEE_SPLIT_PRECISION;
    let expected_fee_relayer = bid_amount.saturating_sub(expected_fee_router)
        * express_relay_metadata_acc.split_relayer
        / FEE_SPLIT_PRECISION;
    let expected_fee_express_relay = bid_amount
        .saturating_sub(expected_fee_router)
        .saturating_sub(expected_fee_relayer);

    assert_eq!(
        balance_router_post - balance_router_pre,
        expected_fee_router
    );
    assert_eq!(
        balance_fee_receiver_relayer_post - balance_fee_receiver_relayer_pre,
        expected_fee_relayer
    );
    assert_eq!(
        balance_express_relay_metadata_post - balance_express_relay_metadata_pre,
        expected_fee_express_relay
    );
    assert_eq!(
        balance_searcher_pre - balance_searcher_post,
        bid_amount + TX_FEE + Rent::default().minimum_balance(RESERVE_ACCOUNTING)
    );
}

#[test]
fn test_bid_with_secondary_relayer() {
    let BidSetupResult {
        mut svm,
        secondary_relayer_signer,
        searcher,
        fee_receiver_relayer,
        router,
        permission_key,
        bid_amount,
        deadline,
        ixs,
        ..
    } = setup_bid();

    let bid_ixs = bid_instructions(
        &secondary_relayer_signer,
        &searcher,
        router,
        fee_receiver_relayer.pubkey(),
        permission_key,
        bid_amount,
        deadline,
        &ixs,
    );

    let express_relay_metadata_key = get_express_relay_metadata_key();

    let balance_router_pre = get_balance(&svm, &router);
    let balance_fee_receiver_relayer_pre = get_balance(&svm, &fee_receiver_relayer.pubkey());
    let balance_express_relay_metadata_pre = get_balance(&svm, &express_relay_metadata_key);
    let balance_searcher_pre = get_balance(&svm, &searcher.pubkey());

    submit_transaction(
        &mut svm,
        &bid_ixs,
        &searcher,
        &[&searcher, &secondary_relayer_signer],
    )
    .expect("Transaction failed unexpectedly");

    let balance_router_post = get_balance(&svm, &router);
    let balance_fee_receiver_relayer_post = get_balance(&svm, &fee_receiver_relayer.pubkey());
    let balance_express_relay_metadata_post = get_balance(&svm, &express_relay_metadata_key);
    let balance_searcher_post = get_balance(&svm, &searcher.pubkey());

    let express_relay_metadata_acc = get_express_relay_metadata(&mut svm);
    let expected_fee_router =
        bid_amount * express_relay_metadata_acc.split_router_default / FEE_SPLIT_PRECISION;
    let expected_fee_relayer = bid_amount.saturating_sub(expected_fee_router)
        * express_relay_metadata_acc.split_relayer
        / FEE_SPLIT_PRECISION;
    let expected_fee_express_relay = bid_amount
        .saturating_sub(expected_fee_router)
        .saturating_sub(expected_fee_relayer);

    assert_eq!(
        balance_router_post - balance_router_pre,
        expected_fee_router
    );
    assert_eq!(
        balance_fee_receiver_relayer_post - balance_fee_receiver_relayer_pre,
        expected_fee_relayer
    );
    assert_eq!(
        balance_express_relay_metadata_post - balance_express_relay_metadata_pre,
        expected_fee_express_relay
    );
    assert_eq!(
        balance_searcher_pre - balance_searcher_post,
        bid_amount + TX_FEE + Rent::default().minimum_balance(RESERVE_ACCOUNTING)
    );
}

#[test]
fn test_bid_fail_wrong_relayer_signer() {
    let BidSetupResult {
        mut svm,
        searcher,
        fee_receiver_relayer,
        router,
        permission_key,
        bid_amount,
        deadline,
        ..
    } = setup_bid();

    let wrong_relayer_signer = Keypair::new();

    let bid_ixs = bid_instructions(
        &wrong_relayer_signer,
        &searcher,
        router,
        fee_receiver_relayer.pubkey(),
        permission_key,
        bid_amount,
        deadline,
        &[],
    );

    let tx_result = submit_transaction(
        &mut svm,
        &bid_ixs,
        &searcher,
        &[&searcher, &wrong_relayer_signer],
    )
    .expect_err("Transaction should have failed");

    assert_custom_error(
        tx_result.err,
        0,
        InstructionError::Custom(AnchorErrorCode::ConstraintHasOne.into()),
    );
}

#[test]
fn test_bid_fail_wrong_relayer_fee_receiver() {
    let BidSetupResult {
        mut svm,
        relayer_signer,
        searcher,
        router,
        permission_key,
        bid_amount,
        deadline,
        ..
    } = setup_bid();

    let wrong_fee_receiver_relayer = Keypair::new();

    let bid_ixs = bid_instructions(
        &relayer_signer,
        &searcher,
        router,
        wrong_fee_receiver_relayer.pubkey(),
        permission_key,
        bid_amount,
        deadline,
        &[],
    );

    let tx_result =
        submit_transaction(&mut svm, &bid_ixs, &searcher, &[&searcher, &relayer_signer])
            .expect_err("Transaction should have failed");

    assert_custom_error(
        tx_result.err,
        0,
        InstructionError::Custom(AnchorErrorCode::ConstraintHasOne.into()),
    );
}

#[test]
fn test_bid_fail_insufficient_searcher_rent() {
    let BidSetupResult {
        mut svm,
        relayer_signer,
        searcher,
        fee_receiver_relayer,
        router,
        permission_key,
        deadline,
        ..
    } = setup_bid();

    let wrong_bid_amount =
        get_balance(&svm, &searcher.pubkey()) - Rent::default().minimum_balance(0) + 1;

    let bid_ixs = bid_instructions(
        &relayer_signer,
        &searcher,
        router,
        fee_receiver_relayer.pubkey(),
        permission_key,
        wrong_bid_amount,
        deadline,
        &[],
    );

    let tx_result =
        submit_transaction(&mut svm, &bid_ixs, &searcher, &[&searcher, &relayer_signer])
            .expect_err("Transaction should have failed");

    assert_custom_error(
        tx_result.err,
        0,
        InstructionError::Custom(ErrorCode::InsufficientSearcherFunds.into()),
    );
}

#[test]
fn test_bid_fail_insufficient_router_rent() {
    let BidSetupResult {
        mut svm,
        relayer_signer,
        searcher,
        fee_receiver_relayer,
        router,
        permission_key,
        deadline,
        ..
    } = setup_bid();

    let wrong_bid_amount = 100;

    let bid_ixs = bid_instructions(
        &relayer_signer,
        &searcher,
        router,
        fee_receiver_relayer.pubkey(),
        permission_key,
        wrong_bid_amount,
        deadline,
        &[],
    );

    let tx_result =
        submit_transaction(&mut svm, &bid_ixs, &searcher, &[&searcher, &relayer_signer])
            .expect_err("Transaction should have failed");

    assert_custom_error(
        tx_result.err,
        0,
        InstructionError::Custom(ErrorCode::InsufficientRent.into()),
    );
}

#[test]
fn test_bid_fail_insufficient_relayer_fee_receiver_rent() {
    let BidSetupResult {
        mut svm,
        relayer_signer,
        searcher,
        fee_receiver_relayer,
        router,
        permission_key,
        deadline,
        ..
    } = setup_bid();

    let wrong_bid_amount = 100;
    let balance_fee_receiver_relayer = get_balance(&svm, &fee_receiver_relayer.pubkey());
    // transfer the fee receiver relayer's balance to the router so the InsufficientRent error is not tirggered for the relayer
    submit_transaction(
        &mut svm,
        &[transfer(
            &fee_receiver_relayer.pubkey(),
            &router,
            balance_fee_receiver_relayer,
        )],
        &relayer_signer,
        &[&relayer_signer, &fee_receiver_relayer],
    )
    .expect("Transaction should have succeeded");

    let bid_ixs = bid_instructions(
        &relayer_signer,
        &searcher,
        router,
        fee_receiver_relayer.pubkey(),
        permission_key,
        wrong_bid_amount,
        deadline,
        &[],
    );

    let tx_result =
        submit_transaction(&mut svm, &bid_ixs, &searcher, &[&searcher, &relayer_signer])
            .expect_err("Transaction should have failed");

    assert_custom_error(
        tx_result.err,
        0,
        InstructionError::Custom(ErrorCode::InsufficientRent.into()),
    );
}

#[test]
fn test_bid_fail_passed_deadline() {
    let BidSetupResult {
        mut svm,
        relayer_signer,
        searcher,
        fee_receiver_relayer,
        router,
        permission_key,
        bid_amount,
        deadline,
        ..
    } = setup_bid();

    let bid_ixs = bid_instructions(
        &relayer_signer,
        &searcher,
        router,
        fee_receiver_relayer.pubkey(),
        permission_key,
        bid_amount,
        deadline,
        &[],
    );

    warp_to_unix(&mut svm, deadline + 1);

    let tx_result =
        submit_transaction(&mut svm, &bid_ixs, &searcher, &[&searcher, &relayer_signer])
            .expect_err("Transaction should have failed");

    assert_custom_error(
        tx_result.err,
        0,
        InstructionError::Custom(ErrorCode::DeadlinePassed.into()),
    );
}

#[test]
fn test_bid_fail_wrong_permission_key() {
    let BidSetupResult {
        mut svm,
        relayer_signer,
        searcher,
        fee_receiver_relayer,
        router,
        permission_key: _,
        bid_amount,
        deadline,
        ixs,
        ..
    } = setup_bid();

    let wrong_permission_key = Keypair::new().pubkey();

    let bid_ixs = bid_instructions(
        &relayer_signer,
        &searcher,
        router,
        fee_receiver_relayer.pubkey(),
        wrong_permission_key,
        bid_amount,
        deadline,
        &ixs,
    );

    let tx_result =
        submit_transaction(&mut svm, &bid_ixs, &searcher, &[&searcher, &relayer_signer])
            .expect_err("Transaction should have failed");

    assert_custom_error(
        tx_result.err,
        1,
        InstructionError::Custom(ErrorCode::MissingPermission.into()),
    );
}

#[test]
fn test_bid_fail_wrong_router_key() {
    let BidSetupResult {
        mut svm,
        relayer_signer,
        searcher,
        fee_receiver_relayer,
        permission_key,
        bid_amount,
        deadline,
        ixs,
        ..
    } = setup_bid();

    let wrong_router = Keypair::new().pubkey();

    let bid_ixs = bid_instructions(
        &relayer_signer,
        &searcher,
        wrong_router,
        fee_receiver_relayer.pubkey(),
        permission_key,
        bid_amount,
        deadline,
        &ixs,
    );

    let tx_result =
        submit_transaction(&mut svm, &bid_ixs, &searcher, &[&searcher, &relayer_signer])
            .expect_err("Transaction should have failed");

    assert_custom_error(
        tx_result.err,
        1,
        InstructionError::Custom(ErrorCode::MissingPermission.into()),
    );
}

#[test]
fn test_bid_fail_no_permission() {
    let BidSetupResult {
        mut svm,
        searcher,
        ixs,
        ..
    } = setup_bid();

    let tx_result = submit_transaction(&mut svm, &ixs, &searcher, &[&searcher])
        .expect_err("Transaction should have failed");

    assert_custom_error(
        tx_result.err,
        0,
        InstructionError::Custom(ErrorCode::MissingPermission.into()),
    );
}

#[test]
fn test_bid_fail_duplicate_permission() {
    let BidSetupResult {
        mut svm,
        relayer_signer,
        searcher,
        fee_receiver_relayer,
        router,
        permission_key,
        bid_amount,
        deadline,
        ..
    } = setup_bid();

    let permission_ix_0 = bid_instructions(
        &relayer_signer,
        &searcher,
        router,
        fee_receiver_relayer.pubkey(),
        permission_key,
        bid_amount,
        deadline,
        &[],
    );

    let bid_ixs = bid_instructions(
        &relayer_signer,
        &searcher,
        router,
        fee_receiver_relayer.pubkey(),
        permission_key,
        bid_amount,
        deadline,
        &permission_ix_0,
    );

    let tx_result =
        submit_transaction(&mut svm, &bid_ixs, &searcher, &[&searcher, &relayer_signer])
            .expect_err("Transaction should have failed");

    assert_custom_error(
        tx_result.err,
        0,
        InstructionError::Custom(ErrorCode::MultiplePermissions.into()),
    );
}
