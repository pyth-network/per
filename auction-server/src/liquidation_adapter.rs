use {
    crate::{
        api::liquidation::OpportunityBid,
        state::VerifiedLiquidationOpportunity,
    },
    anyhow::{
        anyhow,
        Result,
    },
    ethers::{
        abi::Tokenizable,
        contract::{
            abigen,
            ContractRevert,
        },
        core::{
            abi,
            utils::keccak256,
        },
        providers::{
            Http,
            Provider,
        },
        types::{
            Address,
            Bytes,
            RecoveryMessage,
            Signature,
            H256,
            U256,
        },
    },
    std::sync::Arc,
};

abigen!(
    LiquidationAdapter,
    "../per_multicall/out/LiquidationAdapter.sol/LiquidationAdapter.json"
);
abigen!(ERC20, "../per_multicall/out/ERC20.sol/ERC20.json");
abigen!(WETH9, "../per_multicall/out/WETH9.sol/WETH9.json");

impl From<(Address, U256)> for TokenQty {
    fn from(token: (Address, U256)) -> Self {
        TokenQty {
            token:  token.0,
            amount: token.1,
        }
    }
}

pub fn verify_signature(params: liquidation_adapter::LiquidationCallParams) -> Result<()> {
    // this should reflect the verifyCalldata function in the LiquidationAdapter contract
    let data = Bytes::from(abi::encode(&[
        params.repay_tokens.into_token(),
        params.expected_receipt_tokens.into_token(),
        params.contract_address.into_token(),
        params.data.into_token(),
        params.value.into_token(),
        params.bid.into_token(),
    ]));
    // encode packed does not work correctly for U256 so we need to convert it to bytes first
    let nonce_bytes = Bytes::from(<[u8; 32]>::from(params.valid_until));
    let digest = H256(keccak256(abi::encode_packed(&[
        data.into_token(),
        nonce_bytes.into_token(),
    ])?));
    let signature = Signature::try_from(params.signature_liquidator.to_vec().as_slice())
        .map_err(|_x| anyhow!("Error reading signature"))?;
    let signer = signature
        .recover(RecoveryMessage::Hash(digest))
        .map_err(|x| anyhow!(x.to_string()))?;
    let is_matched = signer == params.liquidator;
    is_matched.then_some(()).ok_or_else(|| {
        anyhow!(format!(
            "Invalid signature. Expected signer: {}, Got: {}",
            params.liquidator, signer
        ))
    })
}

pub fn parse_revert_error(revert: &Bytes) -> Option<String> {
    let apdapter_decoded = liquidation_adapter::LiquidationAdapterErrors::decode_with_selector(
        revert,
    )
    .map(|decoded_error| {
        format!(
            "Liquidation Adapter Contract Revert Error: {:#?}",
            decoded_error
        )
    });
    let erc20_decoded = erc20::ERC20Errors::decode_with_selector(revert)
        .map(|decoded_error| format!("ERC20 Contract Revert Error: {:#?}", decoded_error));
    apdapter_decoded.or(erc20_decoded)
}

pub async fn make_liquidator_calldata(
    opportunity: VerifiedLiquidationOpportunity,
    bid: OpportunityBid,
    provider: Provider<Http>,
    adapter_contract: Address,
) -> Result<Bytes> {
    let params = liquidation_adapter::LiquidationCallParams {
        repay_tokens:            opportunity
            .repay_tokens
            .into_iter()
            .map(TokenQty::from)
            .collect(),
        expected_receipt_tokens: opportunity
            .receipt_tokens
            .into_iter()
            .map(TokenQty::from)
            .collect(),
        liquidator:              bid.liquidator,
        contract_address:        opportunity.contract,
        data:                    opportunity.calldata,
        value:                   opportunity.value,
        valid_until:             bid.valid_until,
        bid:                     bid.amount,
        signature_liquidator:    bid.signature.to_vec().into(),
    };
    let client = Arc::new(provider);
    verify_signature(params.clone())?;

    let calldata = LiquidationAdapter::new(adapter_contract, client.clone())
        .call_liquidation(params)
        .calldata()
        .ok_or(anyhow!(
            "Failed to generate calldata for liquidation adapter"
        ))?;

    Ok(calldata)
}
