use solana_sdk::signer::Signer;
use testing::{express_relay::helpers::get_express_relay_metadata, setup::{setup, SetupParams}};

#[test]
fn test_initialize() {
    let split_protocol_default: u64 = 4000;
    let split_relayer: u64 = 2000;

    let setup_params = SetupParams {
        split_protocol_default,
        split_relayer,
    };
    let setup_result = setup(setup_params);

    let express_relay_metadata = get_express_relay_metadata(setup_result.svm);

    assert_eq!(express_relay_metadata.admin, setup_result.admin.pubkey());
    assert_eq!(express_relay_metadata.relayer_signer, setup_result.relayer_signer.pubkey());
    assert_eq!(express_relay_metadata.fee_receiver_relayer, setup_result.fee_receiver_relayer.pubkey());
    assert_eq!(express_relay_metadata.split_protocol_default, split_protocol_default);
    assert_eq!(express_relay_metadata.split_relayer, split_relayer);
}
