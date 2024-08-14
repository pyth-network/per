use solana_sdk::{signature::Keypair, signer::Signer};
use testing::{express_relay::{helpers::get_express_relay_metadata, set_admin::get_set_admin_instruction}, helpers::submit_transaction, setup::{setup, SetupParams}};

#[test]
fn test_set_admin() {
    let setup_result = setup(SetupParams {
        split_protocol_default: 4000,
        split_relayer: 2000,
    });

    let mut svm = setup_result.svm;
    let admin = setup_result.admin;

    let admin_new = Keypair::new();
    let set_admin_ix = get_set_admin_instruction(&admin, admin_new.pubkey());
    submit_transaction(&mut svm, &[set_admin_ix], &admin, &[&admin]).expect("Transaction failed unexpectedly");

    let express_relay_metadata = get_express_relay_metadata(svm);

    assert_eq!(express_relay_metadata.admin, admin_new.pubkey());
}
