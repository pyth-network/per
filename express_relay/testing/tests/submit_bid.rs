use express_relay::{state::FEE_SPLIT_PRECISION, error::ErrorCode};
use anchor_lang::error::ErrorCode as AnchorErrorCode;
use solana_sdk::{instruction::Instruction, native_token::LAMPORTS_PER_SOL, pubkey::Pubkey, rent::Rent, signature::Keypair, signer::Signer};
use testing::{dummy::do_nothing::get_do_nothing_instruction, express_relay::{helpers::{get_express_relay_metadata, get_express_relay_metadata_key, get_protocol_fee_receiver_key}, submit_bid::get_bid_instructions}, helpers::{assert_custom_error, get_balance, submit_transaction, warp_to_unix, TX_FEE}, setup::{setup, SetupParams}};

pub struct BidInfo {
    pub svm: litesvm::LiteSVM,
    pub relayer_signer: Keypair,
    pub searcher: Keypair,
    pub fee_receiver_relayer: Keypair,
    pub protocol: Pubkey,
    pub fee_receiver_protocol: Pubkey,
    pub permission_key: Pubkey,
    pub bid_amount: u64,
    pub deadline: i64,
    pub ixs: Vec<Instruction>,
}

pub const SPLIT_PROTOCOL_DEFAULT: u64 = 4000;
pub const SPLIT_RELAYER: u64 = 2000;

fn setup_bid() -> BidInfo {
    let setup_result = setup(SetupParams {
        split_protocol_default: SPLIT_PROTOCOL_DEFAULT,
        split_relayer: SPLIT_RELAYER,
    }).expect("setup failed");

    let svm = setup_result.svm;
    let relayer_signer = setup_result.relayer_signer;
    let searcher = setup_result.searcher;
    let fee_receiver_relayer = setup_result.fee_receiver_relayer;
    let protocol = dummy::ID;
    let fee_receiver_protocol = get_protocol_fee_receiver_key(protocol);
    let permission_key = Keypair::new().pubkey();
    let bid_amount = 1*LAMPORTS_PER_SOL;
    let deadline: i64 = 100_000_000_000;
    let ixs = [
        get_do_nothing_instruction(&searcher, permission_key)
    ];

    return BidInfo {
        svm,
        relayer_signer,
        searcher,
        fee_receiver_relayer,
        protocol,
        fee_receiver_protocol,
        permission_key,
        bid_amount,
        deadline,
        ixs: ixs.to_vec()
    };
}

#[test]
fn test_bid() {
    let BidInfo {
        mut svm,
        relayer_signer,
        searcher,
        fee_receiver_relayer,
        protocol,
        fee_receiver_protocol,
        permission_key,
        bid_amount,
        deadline,
        ixs
    } = setup_bid();

    let bid_ixs = get_bid_instructions(
        &relayer_signer,
        &searcher,
        protocol,
        fee_receiver_relayer.pubkey(),
        fee_receiver_protocol,
        permission_key,
        bid_amount,
        deadline,
        &ixs
    );

    let express_relay_metadata_key = get_express_relay_metadata_key();

    let balance_fee_receiver_protocol_pre = get_balance(&svm, &fee_receiver_protocol);
    let balance_fee_receiver_relayer_pre = get_balance(&svm, &fee_receiver_relayer.pubkey());
    let balance_express_relay_metadata_pre = get_balance(&svm, &express_relay_metadata_key);
    let balance_searcher_pre = get_balance(&svm, &searcher.pubkey());

    submit_transaction(&mut svm, &bid_ixs, &searcher, &[&searcher, &relayer_signer]).expect("Transaction failed unexpectedly");

    let balance_fee_receiver_protocol_post = get_balance(&svm, &fee_receiver_protocol);
    let balance_fee_receiver_relayer_post = get_balance(&svm, &fee_receiver_relayer.pubkey());
    let balance_express_relay_metadata_post = get_balance(&svm, &express_relay_metadata_key);
    let balance_searcher_post = get_balance(&svm, &searcher.pubkey());

    let express_relay_metadata_acc = get_express_relay_metadata(svm);
    let expected_fee_protocol = bid_amount * express_relay_metadata_acc.split_protocol_default / FEE_SPLIT_PRECISION;
    let expected_fee_relayer = bid_amount.saturating_sub(expected_fee_protocol) * express_relay_metadata_acc.split_relayer / FEE_SPLIT_PRECISION;
    let expected_fee_express_relay = bid_amount.saturating_sub(expected_fee_protocol).saturating_sub(expected_fee_relayer);

    assert_eq!(balance_fee_receiver_protocol_post - balance_fee_receiver_protocol_pre, expected_fee_protocol);
    assert_eq!(balance_fee_receiver_relayer_post - balance_fee_receiver_relayer_pre, expected_fee_relayer);
    assert_eq!(balance_express_relay_metadata_post - balance_express_relay_metadata_pre, expected_fee_express_relay);
    assert_eq!(balance_searcher_pre - balance_searcher_post, bid_amount+TX_FEE);
}

#[test]
fn test_bid_fail_wrong_relayer_signer() {
    let BidInfo {
        mut svm,
        relayer_signer: _,
        searcher,
        fee_receiver_relayer,
        protocol,
        fee_receiver_protocol,
        permission_key,
        bid_amount,
        deadline,
        ixs: _
    } = setup_bid();

    let wrong_relayer_signer = Keypair::new();

    let bid_ixs = get_bid_instructions(
        &wrong_relayer_signer,
        &searcher,
        protocol,
        fee_receiver_relayer.pubkey(),
        fee_receiver_protocol,
        permission_key,
        bid_amount,
        deadline,
        &[]
    );

    let tx_result = submit_transaction(&mut svm, &bid_ixs, &searcher, &[&searcher, &wrong_relayer_signer]).expect_err("Transaction should have failed");

    assert_custom_error(tx_result.err, 0, AnchorErrorCode::ConstraintHasOne.into());
}

#[test]
fn test_bid_fail_wrong_relayer_fee_receiver() {
    let BidInfo {
        mut svm,
        relayer_signer,
        searcher,
        fee_receiver_relayer: _,
        protocol,
        fee_receiver_protocol,
        permission_key,
        bid_amount,
        deadline,
        ixs: _
    } = setup_bid();

    let wrong_fee_receiver_relayer = Keypair::new();

    let bid_ixs = get_bid_instructions(
        &relayer_signer,
        &searcher,
        protocol,
        wrong_fee_receiver_relayer.pubkey(),
        fee_receiver_protocol,
        permission_key,
        bid_amount,
        deadline,
        &[]
    );

    let tx_result = submit_transaction(&mut svm, &bid_ixs, &searcher, &[&searcher, &relayer_signer]).expect_err("Transaction should have failed");

    assert_custom_error(tx_result.err, 0, AnchorErrorCode::ConstraintHasOne.into());
}

#[test]
fn test_bid_fail_insufficient_searcher_rent() {
    let BidInfo {
        mut svm,
        relayer_signer,
        searcher,
        fee_receiver_relayer,
        protocol,
        fee_receiver_protocol,
        permission_key,
        bid_amount: _,
        deadline,
        ixs: _
    } = setup_bid();

    let wrong_bid_amount = get_balance(&svm, &searcher.pubkey()) - Rent::default().minimum_balance(0) + 1;

    let bid_ixs = get_bid_instructions(
        &relayer_signer,
        &searcher,
        protocol,
        fee_receiver_relayer.pubkey(),
        fee_receiver_protocol,
        permission_key,
        wrong_bid_amount,
        deadline,
        &[]
    );

    let tx_result = submit_transaction(&mut svm, &bid_ixs, &searcher, &[&searcher, &relayer_signer]).expect_err("Transaction should have failed");

    assert_custom_error(tx_result.err, 0, ErrorCode::InsufficientSearcherFunds.into());
}

#[test]
fn test_bid_fail_passed_deadline() {
    let BidInfo {
        mut svm,
        relayer_signer,
        searcher,
        fee_receiver_relayer,
        protocol,
        fee_receiver_protocol,
        permission_key,
        bid_amount,
        deadline,
        ixs: _
    } = setup_bid();

    let bid_ixs = get_bid_instructions(
        &relayer_signer,
        &searcher,
        protocol,
        fee_receiver_relayer.pubkey(),
        fee_receiver_protocol,
        permission_key,
        bid_amount,
        deadline,
        &[]
    );

    warp_to_unix(&mut svm, deadline+1);

    let tx_result = submit_transaction(&mut svm, &bid_ixs, &searcher, &[&searcher, &relayer_signer]).expect_err("Transaction should have failed");

    assert_custom_error(tx_result.err, 0, ErrorCode::DeadlinePassed.into());
}

#[test]
fn test_bid_fail_wrong_permission_key() {
    let BidInfo {
        mut svm,
        relayer_signer,
        searcher,
        fee_receiver_relayer,
        protocol,
        fee_receiver_protocol,
        permission_key: _,
        bid_amount,
        deadline,
        ixs
    } = setup_bid();

    let wrong_permission_key = Keypair::new().pubkey();

    let bid_ixs = get_bid_instructions(
        &relayer_signer,
        &searcher,
        protocol,
        fee_receiver_relayer.pubkey(),
        fee_receiver_protocol,
        wrong_permission_key,
        bid_amount,
        deadline,
        &ixs
    );

    let tx_result = submit_transaction(&mut svm, &bid_ixs, &searcher, &[&searcher, &relayer_signer]).expect_err("Transaction should have failed");

    assert_custom_error(tx_result.err, 1, ErrorCode::MissingPermission.into());
}

#[test]
fn test_bid_fail_wrong_protocol_key() {
    let BidInfo {
        mut svm,
        relayer_signer,
        searcher,
        fee_receiver_relayer,
        protocol: _,
        fee_receiver_protocol: _,
        permission_key,
        bid_amount,
        deadline,
        ixs
    } = setup_bid();

    let wrong_protocol = Keypair::new().pubkey();
    let wrong_fee_receiver_protocol = wrong_protocol;

    let bid_ixs = get_bid_instructions(
        &relayer_signer,
        &searcher,
        wrong_protocol,
        fee_receiver_relayer.pubkey(),
        wrong_fee_receiver_protocol,
        permission_key,
        bid_amount,
        deadline,
        &ixs
    );

    let tx_result = submit_transaction(&mut svm, &bid_ixs, &searcher, &[&searcher, &relayer_signer]).expect_err("Transaction should have failed");

    assert_custom_error(tx_result.err, 1, ErrorCode::MissingPermission.into());
}

#[test]
fn test_bid_fail_wrong_fee_receiver_protocol_key() {
    let BidInfo {
        mut svm,
        relayer_signer,
        searcher,
        fee_receiver_relayer,
        protocol,
        fee_receiver_protocol: _,
        permission_key,
        bid_amount,
        deadline,
        ixs: _
    } = setup_bid();

    let wrong_fee_receiver_protocol = Keypair::new().pubkey();

    let bid_ixs = get_bid_instructions(
        &relayer_signer,
        &searcher,
        protocol,
        fee_receiver_relayer.pubkey(),
        wrong_fee_receiver_protocol,
        permission_key,
        bid_amount,
        deadline,
        &[]
    );

    let tx_result = submit_transaction(&mut svm, &bid_ixs, &searcher, &[&searcher, &relayer_signer]).expect_err("Transaction should have failed");

    assert_custom_error(tx_result.err, 0, ErrorCode::InvalidPDAProvided.into());
}

#[test]
fn test_bid_fail_no_permission() {
    let BidInfo {
        mut svm,
        relayer_signer: _,
        searcher,
        fee_receiver_relayer: _,
        protocol: _,
        fee_receiver_protocol: _,
        permission_key: _,
        bid_amount: _,
        deadline: _,
        ixs
    } = setup_bid();

    let tx_result = submit_transaction(&mut svm, &ixs, &searcher, &[&searcher]).expect_err("Transaction should have failed");

    assert_custom_error(tx_result.err, 0, ErrorCode::MissingPermission.into());
}

#[test]
fn test_bid_fail_duplicate_permission() {
    let BidInfo {
        mut svm,
        relayer_signer,
        searcher,
        fee_receiver_relayer,
        protocol,
        fee_receiver_protocol,
        permission_key,
        bid_amount,
        deadline,
        ixs: _
    } = setup_bid();

    let permission_ix_0 = get_bid_instructions(
        &relayer_signer,
        &searcher,
        protocol,
        fee_receiver_relayer.pubkey(),
        fee_receiver_protocol,
        permission_key,
        bid_amount,
        deadline,
        &[]
    );

    let bid_ixs = get_bid_instructions(
        &relayer_signer,
        &searcher,
        protocol,
        fee_receiver_relayer.pubkey(),
        fee_receiver_protocol,
        permission_key,
        bid_amount,
        deadline,
        &permission_ix_0
    );

    let tx_result = submit_transaction(&mut svm, &bid_ixs, &searcher, &[&searcher, &relayer_signer]).expect_err("Transaction should have failed");

    assert_custom_error(tx_result.err, 0, ErrorCode::MultiplePermissions.into());
}
