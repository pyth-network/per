use {
    crate::{
        api::{
            bid::{
                handle_bid,
                BidResult,
            },
            ErrorBodyResponse,
            RestError,
        },
        config::ChainId,
        liquidation_adapter::{
            make_liquidator_calldata,
            parse_revert_error,
            verify_opportunity,
        },
        state::{
            LiquidationOpportunity,
            OpportunityParams,
            Store,
            UnixTimestamp,
        },
    },
    axum::{
        extract::{
            Path,
            Query,
            State,
        },
        Json,
    },
    ethers::{
        abi::Address,
        core::types::Signature,
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
    std::{
        sync::Arc,
        time::{
            SystemTime,
            UNIX_EPOCH,
        },
    },
    utoipa::{
        IntoParams,
        ToResponse,
        ToSchema,
    },
    uuid::Uuid,
};


/// Similar to OpportunityParams, but with the opportunity id included.
#[derive(Serialize, Deserialize, ToSchema, Clone, ToResponse)]
pub struct OpportunityParamsWithId {
    /// The opportunity unique id
    #[schema(example = "f47ac10b-58cc-4372-a567-0e02b2c3d479", value_type=String)]
    opportunity_id: Uuid,
    /// opportunity data
    #[serde(flatten)]
    params:         OpportunityParams,
}

/// Submit a liquidation opportunity ready to be executed.
///
/// The opportunity will be verified by the server. If the opportunity is valid, it will be stored in the database
/// and will be available for bidding.
#[utoipa::path(post, path = "/v1/liquidation/opportunities", request_body = OpportunityParams, responses(
    (status = 200, description = "The created opportunity", body = OpportunityParamsWithId),
    (status = 400, response = ErrorBodyResponse),
    (status = 404, description = "Chain id was not found", body = ErrorBodyResponse),
),)]
pub async fn post_opportunity(
    State(store): State<Arc<Store>>,
    Json(versioned_params): Json<OpportunityParams>,
) -> Result<Json<OpportunityParamsWithId>, RestError> {
    let params = match versioned_params.clone() {
        OpportunityParams::V1(params) => params,
    };
    let chain_store = store
        .chains
        .get(&params.chain_id)
        .ok_or(RestError::InvalidChainId)?;

    let id = Uuid::new_v4();
    let opportunity = LiquidationOpportunity {
        id,
        creation_time: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| RestError::BadParameters("Invalid system time".to_string()))?
            .as_secs() as UnixTimestamp,
        params: versioned_params.clone(),
        bidders: Default::default(),
    };

    verify_opportunity(params.clone(), chain_store, store.per_operator.address())
        .await
        .map_err(|e| RestError::InvalidOpportunity(e.to_string()))?;

    store
        .liquidation_store
        .opportunities
        .write()
        .await
        .insert(params.permission_key.clone(), opportunity);

    Ok(OpportunityParamsWithId {
        opportunity_id: id,
        params:         versioned_params,
    }
    .into())
}


#[derive(Serialize, Deserialize, IntoParams)]
pub struct ChainIdQueryParams {
    #[param(example = "sepolia", value_type=Option<String>)]
    chain_id: Option<ChainId>,
}

/// Fetch all liquidation opportunities ready to be exectued.
#[utoipa::path(get, path = "/v1/liquidation/opportunities", responses(
    (status = 200, description = "Array of liquidation opportunities ready for bidding", body = Vec<OpportunityParamsWithId>),
    (status = 400, response = ErrorBodyResponse),
    (status = 404, description = "Chain id was not found", body = ErrorBodyResponse),
),
params(ChainIdQueryParams))]
pub async fn get_opportunities(
    State(store): State<Arc<Store>>,
    query_params: Query<ChainIdQueryParams>,
) -> Result<axum::Json<Vec<OpportunityParamsWithId>>, RestError> {
    let opportunities: Vec<OpportunityParamsWithId> = store
        .liquidation_store
        .opportunities
        .read()
        .await
        .values()
        .cloned()
        .map(|opportunity| OpportunityParamsWithId {
            opportunity_id: opportunity.id,
            params:         opportunity.params,
        })
        .filter(|params_with_id| {
            let params = match &params_with_id.params {
                OpportunityParams::V1(params) => params,
            };
            if let Some(chain_id) = &query_params.chain_id {
                params.chain_id == *chain_id
            } else {
                true
            }
        })
        .collect();

    Ok(opportunities.into())
}

#[derive(Serialize, Deserialize, ToSchema, Clone)]
pub struct OpportunityBid {
    /// The opportunity permission key
    #[schema(example = "0xdeadbeefcafe", value_type=String)]
    pub permission_key: Bytes,
    /// The bid amount in wei.
    #[schema(example = "1000000000000000000", value_type=String)]
    #[serde(with = "crate::serde::u256")]
    pub amount:         U256,
    /// How long the bid will be valid for.
    #[schema(example = "1000000000000000000", value_type=String)]
    #[serde(with = "crate::serde::u256")]
    pub valid_until:    U256,
    /// Liquidator address
    #[schema(example = "0x5FbDB2315678afecb367f032d93F642f64180aa2", value_type=String)]
    pub liquidator:     Address,
    #[schema(
        example = "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef12",
        value_type=String
    )]
    #[serde(with = "crate::serde::signature")]
    pub signature:      Signature,
}

/// Bid on liquidation opportunity
#[utoipa::path(post, path = "/v1/liquidation/opportunities/{opportunity_id}/bids", request_body=OpportunityBid,
    params(("opportunity_id", description = "Opportunity id to bid on")), responses(
    (status = 200, description = "Bid Result", body = BidResult, example = json!({"status": "OK"})),
    (status = 400, response = ErrorBodyResponse),
    (status = 404, description = "Opportunity or chain id was not found", body = ErrorBodyResponse),
),)]
pub async fn post_bid(
    State(store): State<Arc<Store>>,
    Path(opportunity_id): Path<Uuid>,
    Json(opportunity_bid): Json<OpportunityBid>,
) -> Result<Json<BidResult>, RestError> {
    let opportunity = store
        .liquidation_store
        .opportunities
        .read()
        .await
        .get(&opportunity_bid.permission_key)
        .ok_or(RestError::OpportunityNotFound)?
        .clone();


    if opportunity.id != opportunity_id {
        return Err(RestError::BadParameters(
            "Invalid opportunity_id".to_string(),
        ));
    }

    // TODO: move this logic to searcher side
    if opportunity.bidders.contains(&opportunity_bid.liquidator) {
        return Err(RestError::BadParameters(
            "Liquidator already bid on this opportunity".to_string(),
        ));
    }

    let params = match &opportunity.params {
        OpportunityParams::V1(params) => params,
    };

    let chain_store = store
        .chains
        .get(&params.chain_id)
        .ok_or(RestError::InvalidChainId)?;

    let per_calldata = make_liquidator_calldata(
        params.clone(),
        opportunity_bid.clone(),
        chain_store.provider.clone(),
        chain_store.config.adapter_contract,
    )
    .await
    .map_err(|e| RestError::BadParameters(e.to_string()))?;
    match handle_bid(
        store.clone(),
        crate::api::bid::Bid {
            permission_key: params.permission_key.clone(),
            chain_id:       params.chain_id.clone(),
            contract:       chain_store.config.adapter_contract,
            calldata:       per_calldata,
            amount:         opportunity_bid.amount,
        },
    )
    .await
    {
        Ok(_) => {
            let mut write_guard = store.liquidation_store.opportunities.write().await;
            let liquidation = write_guard.get_mut(&opportunity_bid.permission_key);
            if let Some(liquidation) = liquidation {
                liquidation.bidders.insert(opportunity_bid.liquidator);
            }
            Ok(BidResult {
                status: "OK".to_string(),
            }
            .into())
        }
        Err(e) => match e {
            RestError::SimulationError { result, reason } => {
                let parsed = parse_revert_error(&result);
                match parsed {
                    Some(decoded) => Err(RestError::BadParameters(decoded)),
                    None => {
                        tracing::info!("Could not parse revert reason: {}", reason);
                        Err(RestError::SimulationError { result, reason })
                    }
                }
            }
            _ => Err(e),
        },
    }
}
