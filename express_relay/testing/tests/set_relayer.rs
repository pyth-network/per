use solana_sdk::{signature::Keypair, signer::Signer};
use testing::{express_relay::{helpers::get_express_relay_metadata, set_relayer::get_set_relayer_instruction}, helpers::submit_transaction, setup::{setup, SetupParams}};

#[test]
fn test_set_relayer() {
    let setup_result = setup(SetupParams {
        split_protocol_default: 4000,
        split_relayer: 2000,
    });

    let mut svm = setup_result.svm;
    let admin = setup_result.admin;

    let relayer_signer_new = Keypair::new().pubkey();
    let fee_receiver_relayer_new = Keypair::new().pubkey();
    let set_relayer_ix = get_set_relayer_instruction(&admin, relayer_signer_new, fee_receiver_relayer_new);
    submit_transaction(&mut svm, &[set_relayer_ix], &admin, &[&admin]).expect("Transaction failed unexpectedly");

    let express_relay_metadata = get_express_relay_metadata(svm);

    assert_eq!(express_relay_metadata.relayer_signer, relayer_signer_new);
    assert_eq!(express_relay_metadata.fee_receiver_relayer, fee_receiver_relayer_new);
}
