use {
    express_relay::SwapArgs,
    solana_sdk::signer::Signer,
    testing::{
        express_relay::submit_bid::bid_instructions,
        helpers::{
            get_spl_balance,
            submit_transaction,
        },
        setup::{
            setup_bid,
            BidInfo,
            IxsType,
        },
    },
};

#[test]
pub fn test_swap() {
    let amount_input = 100u64;
    let amount_output = 1000u64;
    let nonce = 0u64;
    let referral_fee_input = true;
    let referral_fee_ppm = 10_000u64;

    let BidInfo {
        mut svm,
        relayer_signer,
        searcher,
        fee_receiver_relayer,
        router,
        permission_key,
        bid_amount,
        deadline,
        ixs,
        trader,
        tas_searcher,
        tas_trader,
        tas_router,
    } = setup_bid(IxsType::Swap(SwapArgs {
        amount_input,
        amount_output,
        nonce,
        referral_fee_input,
        referral_fee_ppm,
    }));
    let trader = trader.unwrap();

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

    let tas_searcher = tas_searcher.unwrap();
    let tas_trader = tas_trader.unwrap();
    let tas_router = tas_router.unwrap();

    let balance_input_searcher_pre = get_spl_balance(&svm, &tas_searcher.input);
    let balance_output_searcher_pre = get_spl_balance(&svm, &tas_searcher.output);
    let balance_input_trader_pre = get_spl_balance(&svm, &tas_trader.input);
    let balance_output_trader_pre = get_spl_balance(&svm, &tas_trader.output);
    let balance_input_router_pre = get_spl_balance(&svm, &tas_router.input);
    let balance_output_router_pre = get_spl_balance(&svm, &tas_router.output);

    submit_transaction(
        &mut svm,
        &bid_ixs,
        &searcher,
        &[&searcher, &relayer_signer, &trader],
    )
    .expect("Transaction failed unexpectedly");

    let balance_input_searcher_post = get_spl_balance(&svm, &tas_searcher.input);
    let balance_output_searcher_post = get_spl_balance(&svm, &tas_searcher.output);
    let balance_input_trader_post = get_spl_balance(&svm, &tas_trader.input);
    let balance_output_trader_post = get_spl_balance(&svm, &tas_trader.output);
    let balance_input_router_post = get_spl_balance(&svm, &tas_router.input);
    let balance_output_router_post = get_spl_balance(&svm, &tas_router.output);

    let fee_input = (referral_fee_input as u64) * (amount_input * referral_fee_ppm / 1_000_000);
    let fee_output =
        (1 - referral_fee_input as u64) * (amount_output * referral_fee_ppm / 1_000_000);
    assert_eq!(
        balance_input_searcher_pre,
        balance_input_searcher_post + amount_input,
        "Searcher input balance incorrect"
    );
    assert_eq!(
        balance_output_searcher_post,
        balance_output_searcher_pre + amount_output - fee_output,
        "Searcher output balance incorrect"
    );
    assert_eq!(
        balance_input_trader_post,
        balance_input_trader_pre + amount_input - fee_input,
        "Trader input balance incorrect"
    );
    assert_eq!(
        balance_output_trader_pre,
        balance_output_trader_post + amount_output,
        "Trader output balance incorrect"
    );
    assert_eq!(
        balance_input_router_post,
        balance_input_router_pre + fee_input,
        "Router input balance incorrect"
    );
    assert_eq!(
        balance_output_router_pre,
        balance_output_router_post + fee_output,
        "Router output balance incorrect"
    );
}
