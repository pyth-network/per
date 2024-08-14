use solana_sdk::{signature::Keypair, signer::Signer};
use testing::{express_relay::{helpers::get_protocol_config, set_protocol_split::get_set_protocol_split_instruction}, helpers::submit_transaction, setup::{setup, SetupParams}};

#[test]
fn test_set_split() {
    let setup_result = setup(SetupParams {
        split_protocol_default: 4000,
        split_relayer: 2000,
    });

    let mut svm = setup_result.svm;
    let admin = setup_result.admin;

    let protocol = Keypair::new().pubkey();
    let split_protocol: u64 = 5000;
    let set_protocol_split_ix = get_set_protocol_split_instruction(&admin, protocol, split_protocol);
    submit_transaction(&mut svm, &[set_protocol_split_ix], &admin, &[&admin]).expect("Transaction failed unexpectedly");

    let protocol_config = get_protocol_config(svm, protocol).expect("Protocol Config not initialized");

    assert_eq!(protocol_config.protocol, protocol);
    assert_eq!(protocol_config.split, split_protocol);
}
