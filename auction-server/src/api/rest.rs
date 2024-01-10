use crate::api::RestError;
use crate::auction::simulate_bids;
use crate::auction::per::MulticallStatus;
use crate::state::{SimulatedBid, Store, Opportunity, GetOppsParams};
use axum::{extract::State, Json};
use ethers::abi::Address;
use ethers::contract::EthError;
use ethers::middleware::contract::ContractError;

use ethers::signers::Signer;
use ethers::types::{Bytes, U256};
use ethers::utils::hex::FromHex;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;
use axum::extract::Query;

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
    (status = 200, description = "Bid was placed succesfully", body = String),
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

/// Post liquidation opportunities.
///
/// Can only be called by permissioned operator of the beacon, with their permission key
// #[axum_macros::debug_handler]
#[utoipa::path(post, path = "/surface", request_body = Vec<Opportunity>, responses(
    (status = 200, description = "Posted opportunities successfully", body = String),
    (status = 400, response=RestError)
),)]
pub async fn surface(
    State(store): State<Arc<Store>>,
    Json(opps): Json<Vec<Opportunity>>,
) -> Result<String, RestError> {
    for opp in &opps {
        let chain_store = store
            .chains
            .get(&opp.chain_id)
            .ok_or(RestError::InvalidChainId)?;
    
        let contract = opp
            .contract
            .parse::<Address>()
            .map_err(|_| RestError::BadParameters("Invalid contract address".to_string()))?;

        chain_store
            .opps
            .write()
            .await
            .entry(contract.clone())
            .or_default()
            .push(opp.clone());
    }
    Ok(opps.len().to_string())
}

/// Get liquidation opportunities
///
// #[axum_macros::debug_handler]
#[utoipa::path(get, path = "/getOpps", 
    params(
        ("chain_id" = String, Query, description = "Chain ID to retrieve opportunities for"),
        ("contract" = Option<String>, Query, description = "Contract address to filter by")
    ),
    responses(
        (status = 200, description = "Got opportunities successfully", body = String),
        (status = 400, response=RestError)
    )
,)]
pub async fn get_opps(
    State(store): State<Arc<Store>>,
    Query(params): Query<GetOppsParams>
) -> Result<Json<Vec<Opportunity>>, RestError> {    
    let chain_id = params.chain_id;
    let contract = params.contract;
    
    let chain_store = store
        .chains
        .get(&chain_id)
        .ok_or(RestError::InvalidChainId)?;

    let mut opps: Vec<Opportunity> = Default::default();

    match contract {
        Some(ref x) => {
            let key = x
                .parse::<Address>()
                .map_err(|_| RestError::BadParameters("Invalid contract address".to_string()))?;

            opps = chain_store.opps.write().await.entry(key).or_default().to_vec();
        },
        None => {
            let opps_dict = chain_store.opps.write().await;

            for key in opps_dict.keys() {
                let opps_key = chain_store.opps.write().await.entry(key.clone()).or_default().clone();
                opps.extend(opps_key);
            }
        }
    }
    Ok(Json(opps))
}