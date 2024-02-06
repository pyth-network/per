use {
    crate::{
        api::RestError,
        auction::{
            simulate_bids,
            SimulationError,
        },
        state::{
            SimulatedBid,
            Store,
        },
    },
    axum::{
        extract::State,
        Json,
    },
    ethers::{
        abi::Address,
        contract::EthError,
        middleware::contract::ContractError,
        signers::Signer,
        types::{
            Bytes,
            U256,
        },
    },
    serde::{
        Deserialize,
        Serialize,
    },
    std::sync::Arc,
    utoipa::ToSchema,
};

#[derive(Serialize, Deserialize, ToSchema, Clone)]
pub struct Bid {
    /// The permission key to bid on.
    #[schema(example = "0xdeadbeef", value_type=String)]
    pub permission_key: Bytes,
    /// The chain id to bid on.
    #[schema(example = "sepolia")]
    pub chain_id:       String,
    /// The contract address to call.
    #[schema(example = "0xcA11bde05977b3631167028862bE2a173976CA11",value_type = String)]
    pub contract:       Address,
    /// Calldata for the contract call.
    #[schema(example = "0xdeadbeef", value_type=String)]
    pub calldata:       Bytes,
    /// Amount of bid in wei.
    #[schema(example = "10", value_type=String)]
    #[serde(with = "crate::serde::u256")]
    pub amount:         U256,
}

pub async fn handle_bid(store: Arc<Store>, bid: Bid) -> Result<String, RestError> {
    let chain_store = store
        .chains
        .get(&bid.chain_id)
        .ok_or(RestError::InvalidChainId)?;
    let call = simulate_bids(
        store.per_operator.address(),
        chain_store.provider.clone(),
        chain_store.config.clone(),
        bid.permission_key.clone(),
        vec![bid.contract],
        vec![bid.calldata.clone()],
        vec![bid.amount],
    );

    if let Err(e) = call.await {
        return match e {
            SimulationError::LogicalError { result, reason } => {
                Err(RestError::SimulationError { result, reason })
            }
            SimulationError::ContractError(e) => match e {
                ContractError::Revert(reason) => Err(RestError::BadParameters(format!(
                    "Contract Revert Error: {}",
                    String::decode_with_selector(&reason)
                        .unwrap_or("unable to decode revert".to_string())
                ))),
                ContractError::MiddlewareError { e: _ } => Err(RestError::TemporarilyUnavailable),
                ContractError::ProviderError { e: _ } => Err(RestError::TemporarilyUnavailable),
                _ => Err(RestError::BadParameters(format!("Error: {}", e))),
            },
        };
    };

    chain_store
        .bids
        .write()
        .await
        .entry(bid.permission_key.clone())
        .or_default()
        .push(SimulatedBid {
            contract: bid.contract,
            calldata: bid.calldata.clone(),
            bid:      bid.amount,
        });
    Ok("OK".to_string())
}

/// Bid on a specific permission key for a specific chain.
///
/// Your bid will be simulated and verified by the server. Depending on the outcome of the auction, a transaction
/// containing the contract call will be sent to the blockchain expecting the bid amount to be paid after the call.
#[utoipa::path(post, path = "/v1/bid", request_body = Bid, responses(
    (status = 200, description = "Bid was placed succesfully", body = String),
    (status = 400, response=RestError)
),)]
pub async fn bid(
    State(store): State<Arc<Store>>,
    Json(bid): Json<Bid>,
) -> Result<String, RestError> {
    let bid = bid.clone();
    store
        .chains
        .get(&bid.chain_id)
        .ok_or(RestError::InvalidChainId)?;

    handle_bid(store, bid).await
}
