use express_relay::state::FEE_SPLIT_PRECISION;
use solana_sdk::{native_token::LAMPORTS_PER_SOL, signature::Keypair, signer::Signer};
use testing::{dummy::do_nothing::get_do_nothing_instruction, express_relay::{helpers::{get_express_relay_metadata, get_express_relay_metadata_key, get_protocol_fee_receiver_key}, permission::get_permission_instructions}, helpers::{get_balance, submit_transaction, TX_FEE}, setup::{setup, SetupParams}};

#[test]
fn test_permission() {
    let setup_result = setup(SetupParams {
        split_protocol_default: 4000,
        split_relayer: 2000,
    });

    let mut svm = setup_result.svm;
    let relayer_signer = setup_result.relayer_signer;
    let searcher = setup_result.searcher;
    let fee_receiver_relayer = setup_result.fee_receiver_relayer;
    let protocol = dummy::ID;
    let fee_receiver_protocol = get_protocol_fee_receiver_key(protocol);
    let permission_key = Keypair::new().pubkey();
    let bid_amount = 1*LAMPORTS_PER_SOL;
    let deadline = 100_000_000_000;
    let ixs = [
        get_do_nothing_instruction(&searcher, permission_key)
    ];

    let permission_ixs = get_permission_instructions(
        &relayer_signer,
        &searcher,
        dummy::ID,
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

    submit_transaction(&mut svm, &permission_ixs, &searcher, &[&searcher, &relayer_signer]).expect("Transaction failed unexpectedly");

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
