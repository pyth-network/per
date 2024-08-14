use express_relay::state::RESERVE_EXPRESS_RELAY_METADATA;
use solana_sdk::{native_token::LAMPORTS_PER_SOL, signature::Keypair, signer::Signer};
use testing::{express_relay::{helpers::get_express_relay_metadata_key, withdraw_fees::get_withdraw_fees_instruction}, helpers::{get_balance, submit_transaction}, setup::{setup, SetupParams}};

#[test]
fn test_withdraw_fees() {
    let setup_result = setup(SetupParams {
        split_protocol_default: 4000,
        split_relayer: 2000,
    });

    let mut svm = setup_result.svm;
    let admin = setup_result.admin;

    let fee_receiver_admin = Keypair::new();
    let withdraw_fees_ix = get_withdraw_fees_instruction(&admin, fee_receiver_admin.pubkey());
    let express_relay_metadata_key = get_express_relay_metadata_key();
    let total_fees: u64 = 1*LAMPORTS_PER_SOL;
    svm.airdrop(&express_relay_metadata_key, total_fees).unwrap();

    let balance_express_relay_metadata_pre = get_balance(&svm, &express_relay_metadata_key);
    let balance_fee_receiver_admin_pre = get_balance(&svm, &fee_receiver_admin.pubkey());

    submit_transaction(&mut svm, &[withdraw_fees_ix], &admin, &[&admin]).expect("Transaction failed unexpectedly");

    let balance_express_relay_metadata_post = get_balance(&svm, &express_relay_metadata_key);
    let balance_fee_receiver_admin_post = get_balance(&svm, &fee_receiver_admin.pubkey());

    assert_eq!(balance_express_relay_metadata_pre - balance_express_relay_metadata_post, total_fees);
    assert_eq!(balance_fee_receiver_admin_post - balance_fee_receiver_admin_pre, total_fees);
    assert_eq!(balance_express_relay_metadata_post, svm.minimum_balance_for_rent_exemption(RESERVE_EXPRESS_RELAY_METADATA));
}
