use {
    crate::{
        api::{
            bid::BidResult,
            ws::UpdateEvent::NewOpportunity,
            ErrorBodyResponse,
            RestError,
        },
        config::ChainId,
        liquidation_adapter::{
            handle_liquidation_bid,
            verify_opportunity,
            OpportunityBid,
        },
        state::{
            LiquidationOpportunity,
            OpportunityId,
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
    ethers::signers::Signer,
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
pub struct OpportunityParamsWithMetadata {
    /// The opportunity unique id
    #[schema(example = "f47ac10b-58cc-4372-a567-0e02b2c3d479", value_type=String)]
    opportunity_id: OpportunityId,
    /// Creation time of the opportunity
    #[schema(example = 1700000000, value_type=i64)]
    creation_time:  UnixTimestamp,
    /// opportunity data
    #[serde(flatten)]
    // expands params into component fields in the generated client schemas
    #[schema(inline)]
    params:         OpportunityParams,
}

impl OpportunityParamsWithMetadata {
    pub fn get_chain_id(&self) -> &ChainId {
        match &self.params {
            OpportunityParams::V1(params) => &params.chain_id,
        }
    }
}

impl From<LiquidationOpportunity> for OpportunityParamsWithMetadata {
    fn from(val: LiquidationOpportunity) -> Self {
        OpportunityParamsWithMetadata {
            opportunity_id: val.id,
            creation_time:  val.creation_time,
            params:         val.params,
        }
    }
}

/// Submit a liquidation opportunity ready to be executed.
///
/// The opportunity will be verified by the server. If the opportunity is valid, it will be stored in the database
/// and will be available for bidding.
#[utoipa::path(post, path = "/v1/liquidation/opportunities", request_body = OpportunityParams, responses(
    (status = 200, description = "The created opportunity", body = OpportunityParamsWithMetadata),
    (status = 400, response = ErrorBodyResponse),
    (status = 404, description = "Chain id was not found", body = ErrorBodyResponse),
),)]
pub async fn post_opportunity(
    State(store): State<Arc<Store>>,
    Json(versioned_params): Json<OpportunityParams>,
) -> Result<Json<OpportunityParamsWithMetadata>, RestError> {
    let OpportunityParams::V1(params) = versioned_params.clone();
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


    let opportunities_map = &store.liquidation_store.opportunities;
    if let Some(mut opportunities_existing) = opportunities_map.get_mut(&params.permission_key) {
        // check if same opportunity exists in the vector
        for opportunity_existing in opportunities_existing.iter() {
            if opportunity_existing == &opportunity {
                return Err(RestError::BadParameters(
                    "Duplicate opportunity submission".to_string(),
                ));
            }
        }

        opportunities_existing.push(opportunity.clone());
    } else {
        opportunities_map.insert(params.permission_key.clone(), vec![opportunity.clone()]);
    }

    store
        .ws
        .broadcast_sender
        .send(NewOpportunity(opportunity.clone().into()))
        .map_err(|e| {
            tracing::error!("Failed to send update: {}", e);
            RestError::TemporarilyUnavailable
        })?;

    tracing::debug!("number of permission keys: {}", opportunities_map.len());
    tracing::debug!(
        "number of opportunities for key: {}",
        opportunities_map
            .get(&params.permission_key)
            .map(|opps| opps.len())
            .unwrap_or(0)
    );

    let opportunity_with_metadata: OpportunityParamsWithMetadata = opportunity.into();

    Ok(opportunity_with_metadata.into())
}


#[derive(Serialize, Deserialize, IntoParams)]
pub struct ChainIdQueryParams {
    #[param(example = "sepolia", value_type=Option<String>)]
    chain_id: Option<ChainId>,
}

/// Fetch all liquidation opportunities ready to be exectued.
#[utoipa::path(get, path = "/v1/liquidation/opportunities", responses(
    (status = 200, description = "Array of liquidation opportunities ready for bidding", body = Vec<OpportunityParamsWithMetadata>),
    (status = 400, response = ErrorBodyResponse),
    (status = 404, description = "Chain id was not found", body = ErrorBodyResponse),
),
params(ChainIdQueryParams))]
pub async fn get_opportunities(
    State(store): State<Arc<Store>>,
    query_params: Query<ChainIdQueryParams>,
) -> Result<axum::Json<Vec<OpportunityParamsWithMetadata>>, RestError> {
    let opportunities: Vec<OpportunityParamsWithMetadata> = store
        .liquidation_store
        .opportunities
        .iter()
        .map(|opportunities_key| {
            opportunities_key
                .last()
                .expect("A permission key vector should have at least one opportunity")
                .clone()
                .into()
        })
        .filter(|params_with_id: &OpportunityParamsWithMetadata| {
            let OpportunityParams::V1(params) = &params_with_id.params;
            if let Some(chain_id) = &query_params.chain_id {
                params.chain_id == *chain_id
            } else {
                true
            }
        })
        .collect();

    Ok(opportunities.into())
}

/// Bid on liquidation opportunity
#[utoipa::path(post, path = "/v1/liquidation/opportunities/{opportunity_id}/bids", request_body=OpportunityBid,
    params(("opportunity_id"=String, description = "Opportunity id to bid on")), responses(
    (status = 200, description = "Bid Result", body = BidResult, example = json!({"status": "OK"})),
    (status = 400, response = ErrorBodyResponse),
    (status = 404, description = "Opportunity or chain id was not found", body = ErrorBodyResponse),
),)]
pub async fn bid(
    State(store): State<Arc<Store>>,
    Path(opportunity_id): Path<OpportunityId>,
    Json(opportunity_bid): Json<OpportunityBid>,
) -> Result<Json<BidResult>, RestError> {
    match handle_liquidation_bid(store, opportunity_id, &opportunity_bid).await {
        Ok(id) => Ok(BidResult {
            status: "OK".to_string(),
            id,
        }
        .into()),
        Err(e) => Err(e),
    }
}
