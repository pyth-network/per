use testing::{express_relay::{helpers::get_express_relay_metadata, set_splits::get_set_splits_instruction}, helpers::submit_transaction, setup::{setup, SetupParams}};

#[test]
fn test_set_split() {
    let setup_result = setup(SetupParams {
        split_protocol_default: 4000,
        split_relayer: 2000,
    });

    let mut svm = setup_result.svm;
    let admin = setup_result.admin;

    let split_protocol_default_new: u64 = 5000;
    let split_relayer_new: u64 = 1000;
    let set_splits_ix = get_set_splits_instruction(&admin, split_protocol_default_new, split_relayer_new);
    submit_transaction(&mut svm, &[set_splits_ix], &admin, &[&admin]).expect("Transaction failed unexpectedly");

    let express_relay_metadata = get_express_relay_metadata(svm);

    assert_eq!(express_relay_metadata.split_protocol_default, split_protocol_default_new);
    assert_eq!(express_relay_metadata.split_relayer, split_relayer_new);
}
