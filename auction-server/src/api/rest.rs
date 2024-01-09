use crate::api::RestError;
use crate::auction::simulate_bids;
use crate::auction::per::MulticallStatus;
use crate::state::{SimulatedBid, Store};
use axum::extract::State;
use axum::Json;
use ethers::abi::Address;
use ethers::contract::EthError;
use ethers::middleware::contract::ContractError;

use ethers::signers::Signer;
use ethers::types::{Bytes, U256};
use ethers::utils::hex::FromHex;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

#[derive(Serialize, Deserialize, ToSchema, Clone)]
pub struct Bid {
    /// The permission key to bid on.
    #[schema(example = "0xdeadbeef")]
    permission_key: String,
    /// The chain id to bid on.
    #[schema(example = "sepolia")]
    chain_id: String,
    /// The contract address to call.
    #[schema(example = "0xcA11bde05977b3631167028862bE2a173976CA11")]
    contract: String,
    /// Calldata for the contract call.
    #[schema(example = "0xdeadbeef")]
    calldata: String,
    /// Amount of bid in wei.
    #[schema(example = "1000000000000000000")]
    bid: String,
}

/// Bid on a specific permission key for a specific chain.
///
/// Your bid will be simulated and verified by the server. Depending on the outcome of the auction, a transaction containing the contract call will be sent to the blockchain expecting the bid amount to be paid after the call.
#[utoipa::path(post, path = "/bid", request_body = Bid, responses(
    (status = 200, description = "Bid was placed succesfuly", body = String),
    (status = 400, response=RestError)
),)]
pub async fn bid(
    State(store): State<Arc<Store>>,
    Json(bid): Json<Bid>,
) -> Result<String, RestError> {
    let chain_store = store
        .chains
        .get(&bid.chain_id)
        .ok_or(RestError::InvalidChainId)?;

    let permission = Bytes::from_hex(bid.permission_key)
        .map_err(|_| RestError::BadParameters("Invalid permission key".to_string()))?;
    let calldata = Bytes::from_hex(bid.calldata)
        .map_err(|_| RestError::BadParameters("Invalid calldata".to_string()))?;

    let contract = bid
        .contract
        .parse::<Address>()
        .map_err(|_| RestError::BadParameters("Invalid contract address".to_string()))?;

    let bid_amount = bid
        .bid
        .parse::<U256>()
        .map_err(|_| RestError::BadParameters("Invalid bid amount".to_string()))?;

    let call = simulate_bids(
        store.per_operator.address(),
        chain_store.provider.clone(),
        chain_store.config.clone(),
        permission.clone(),
        vec![contract],
        vec![calldata.clone()],
        vec![bid_amount],
    );

    match call.await {
        Ok(result) => {
            let multicall_results: Vec<MulticallStatus> = result;
            if !multicall_results.iter().all(|x| x.external_success) {
                let first_reason = multicall_results
                    .first()
                    .cloned()
                    .unwrap()
                    .multicall_revert_reason;
                let first_result = multicall_results
                    .first()
                    .cloned()
                    .unwrap()
                    .external_result;
                return Err(RestError::BadParameters(format!(
                    "Call Revert: {}, {}",
                    first_result,
                    first_reason
                )));
            }
        }
        Err(e) => {
            return match e {
                ContractError::Revert(reason) => Err(RestError::BadParameters(format!(
                    "Contract Revert Error: {}",
                    String::decode_with_selector(&reason)
                        .unwrap_or("unable to decode revert".to_string())
                ))),
                ContractError::MiddlewareError { e: _ } => Err(RestError::TemporarilyUnavailable),
                ContractError::ProviderError { e: _ } => Err(RestError::TemporarilyUnavailable),
                _ => Err(RestError::BadParameters(format!("Error: {}", e))),
            }
        }
    };

    chain_store
        .bids
        .write()
        .await
        .entry(permission.clone())
        .or_default()
        .push(SimulatedBid {
            contract,
            calldata: calldata.clone(),
            bid: bid_amount,
        });
    Ok(contract.to_string())
}
