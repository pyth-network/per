use {
    crate::{
        api::marketplace::{
            OrderBid,
            VerifiedOrderBid,
        },
        state::VerifiedOrder,
    },
    anyhow::anyhow,
    ethers::{
        abi::{
            AbiEncode,
            Token,
            Tokenizable,
        },
        contract::{
            abigen,
            ContractError,
        },
        core::{
            abi,
            utils::keccak256,
        },
        middleware::{
            transformer::{
                Transformer,
                TransformerError,
            },
            SignerMiddleware,
            TransformerMiddleware,
        },
        providers::{
            Http,
            Provider,
            ProviderError,
        },
        signers::{
            LocalWallet,
            Signer,
        },
        types::{
            transaction::eip2718::TypedTransaction,
            Address,
            Bytes,
            RecoveryMessage,
            Signature,
            TransactionReceipt,
            TransactionRequest,
            H256,
            U256,
        },
    },
    std::{
        sync::{
            atomic::Ordering,
            Arc,
        },
        time::Duration,
    },
};

abigen!(LiquidationAdapter, "src/LiquidationAdapter.json");

fn convert_to_token_qty(tokens: Vec<(Address, U256)>) -> Vec<TokenQty> {
    tokens
        .iter()
        .map(|token| liquidation_adapter::TokenQty {
            token:  token.0,
            amount: token.1,
        })
        .collect()
}

pub fn verify_signature(params: liquidation_adapter::LiquidationCallParams) -> Result<(), String> {
    // this should reflect the verifyCalldata function in the LiquidationAdapter contract
    let data = abi::encode(&[
        params.repay_tokens.into_token(),
        params.expected_receipt_tokens.into_token(),
        params.contract_address.into_token(),
        params.data.into_token(),
        params.bid.into_token(),
    ]);
    let nonce = params.valid_until;
    let digest = H256(keccak256(
        abi::encode_packed(&[data.into_token(), nonce.into_token()]).map_err(|x| x.to_string())?,
    )); // TODO: Fix unwrap
    let signature = Signature::try_from(params.signature_liquidator.to_vec().as_slice())
        .map_err(|x| "Error reading signature")?;
    let signer = signature
        .recover(RecoveryMessage::Hash(digest))
        .map_err(|x| x.to_string())?;
    match params.liquidator == signer {
        true => Ok(()),
        false => Err(format!(
            "Invalid signature. Expected: {}, Got: {}",
            params.liquidator, signer
        )),
    }
}

pub fn make_liquidator_calldata(
    order: VerifiedOrder,
    bid: VerifiedOrderBid,
) -> Result<Bytes, String> {
    let params = liquidation_adapter::LiquidationCallParams {
        repay_tokens:            convert_to_token_qty(order.repay_tokens), // TODO: consistent naming across rust, rest, and solidity
        expected_receipt_tokens: convert_to_token_qty(order.receipt_tokens),
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
