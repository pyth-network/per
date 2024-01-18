use {
    crate::{
        api::RestError,
        auction::simulate_bids,
        state::{
            GetOppsParams,
            Opportunity,
            SimulatedBid,
            Store,
        },
    },
    axum::{
        extract::{
            Query,
            State,
        },
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
        utils::hex::FromHex,
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
    #[schema(example = "0xdeadbeef")]
    permission_key: String,
    /// The chain id to bid on.
    #[schema(example = "sepolia")]
    chain_id:       String,
    /// The contract address to call.
    #[schema(example = "0xcA11bde05977b3631167028862bE2a173976CA11")]
    contract:       String,
    /// Calldata for the contract call.
    #[schema(example = "0xdeadbeef")]
    calldata:       String,
    /// Amount of bid in wei.
    #[schema(example = "1000000000000000000")]
    bid:            String,
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
        Ok(multicall_results) => match multicall_results.first() {
            Some(first_result) => {
                if !multicall_results.iter().all(|x| x.external_success) {
                    return Err(RestError::BadParameters(format!(
                        "Call Revert: Result:{} - Reason:{}",
                        first_result.external_result.clone(),
                        first_result.multicall_revert_reason.clone()
                    )));
                }
            }
            None => {
                return Err(RestError::BadParameters(
                    "No results from multicall".to_string(),
                ))
            }
        },
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
                        
            let opps_contract = chain_store.opps.read().await.get(&key).cloned();

            if let Some(x) = opps_contract {
                opps = x.clone();
            }
        },
        None => {
            let opps_dict = chain_store.opps.read().await;

            opps_dict.iter().for_each(|(_, value)| {
                opps.extend(value.clone());
            });
        }
    }
    Ok(Json(opps))
}