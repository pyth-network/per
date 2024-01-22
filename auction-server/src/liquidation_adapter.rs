use {
    crate::{
        api::marketplace::VerifiedOrderBid,
        state::VerifiedOrder,
    },
    anyhow::{
        anyhow,
        Result,
    },
    ethers::{
        abi::{
            AbiEncode,
            Tokenizable,
        },
        contract::abigen,
        core::{
            abi,
            utils::keccak256,
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
};

abigen!(
    LiquidationAdapter,
    "../per_multicall/out/LiquidationAdapter.sol/LiquidationAdapter.json"
);
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
    let data = abi::encode(&[
        params.repay_tokens.into_token(),
        params.expected_receipt_tokens.into_token(),
        params.contract_address.into_token(),
        params.data.into_token(),
        params.bid.into_token(),
    ]);
    let nonce = params.valid_until;
    let digest = H256(keccak256(abi::encode_packed(&[
        data.into_token(),
        nonce.into_token(),
    ])?));
    let signature = Signature::try_from(params.signature_liquidator.to_vec().as_slice())
        .map_err(|_x| anyhow!("Error reading signature"))?;
    let signer = signature
        .recover(RecoveryMessage::Hash(digest))
        .map_err(|x| anyhow!(x.to_string()))?;
    let is_matched = signer == params.liquidator;
    is_matched.then_some(()).ok_or_else(|| {
        anyhow!(format!(
            "Invalid signature. Expected: {}, Got: {}",
            params.liquidator, signer
        ))
    })
}

pub fn make_liquidator_calldata(order: VerifiedOrder, bid: VerifiedOrderBid) -> Result<Bytes> {
    let params = liquidation_adapter::LiquidationCallParams {
        repay_tokens:            order.repay_tokens.into_iter().map(TokenQty::from).collect(), // TODO: consistent naming across rust, rest, and solidity
        expected_receipt_tokens: order
            .receipt_tokens
            .into_iter()
            .map(TokenQty::from)
            .collect(),
        liquidator:              bid.liquidator,
        contract_address:        order.contract,
        data:                    order.calldata,
        valid_until:             bid.valid_until,
        bid:                     bid.bid_amount,
        signature_liquidator:    bid.signature.to_vec().into(),
    };
    match verify_signature(params.clone()) {
        Ok(_) => Ok(params.encode().into()),
        Err(e) => Err(e),
    }
}
