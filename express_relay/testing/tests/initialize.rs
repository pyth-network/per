use {
    express_relay::{
        error::ErrorCode,
        state::FEE_SPLIT_PRECISION,
    },
    solana_sdk::signer::Signer,
    testing::{
        express_relay::helpers::get_express_relay_metadata,
        helpers::assert_custom_error,
        setup::{
            setup,
            SetupParams,
        },
    },
};

#[test]
fn test_initialize() {
    let split_router_default: u64 = 4000;
    let split_relayer: u64 = 2000;

    let setup_params = SetupParams {
        split_router_default,
        split_relayer,
    };
    let setup_result = setup(setup_params).expect("setup failed");

    let express_relay_metadata = get_express_relay_metadata(setup_result.svm);

    assert_eq!(express_relay_metadata.admin, setup_result.admin.pubkey());
    assert_eq!(
        express_relay_metadata.relayer_signer,
        setup_result.relayer_signer.pubkey()
    );
    assert_eq!(
        express_relay_metadata.fee_receiver_relayer,
        setup_result.fee_receiver_relayer.pubkey()
    );
    assert_eq!(
        express_relay_metadata.split_router_default,
        split_router_default
    );
    assert_eq!(express_relay_metadata.split_relayer, split_relayer);
}

#[test]
fn test_initialize_fail_high_split_router() {
    let split_router_default: u64 = FEE_SPLIT_PRECISION + 1;
    let split_relayer: u64 = 2000;

    let setup_params = SetupParams {
        split_router_default,
        split_relayer,
    };
    let setup_result = setup(setup_params);

    match setup_result {
        Ok(_) => panic!("expected setup to fail"),
        Err(err) => assert_custom_error(err, 0, ErrorCode::FeeSplitLargerThanPrecision.into()),
    }
}

#[test]
fn test_initialize_fail_high_split_relayer() {
    let split_router_default: u64 = 4000;
    let split_relayer: u64 = FEE_SPLIT_PRECISION + 1;

    let setup_params = SetupParams {
        split_router_default,
        split_relayer,
    };
    let setup_result = setup(setup_params);

    match setup_result {
        Ok(_) => panic!("expected setup to fail"),
        Err(err) => assert_custom_error(err, 0, ErrorCode::FeeSplitLargerThanPrecision.into()),
    }
}
