use {
    super::Auth,
    crate::{
        api::{
            bid::BidResult,
            ws::UpdateEvent::NewOpportunity,
            ErrorBodyResponse,
            GetOpportunitiesQueryParams,
            OpportunityMode,
            RestError,
        },
        config::ChainId,
        opportunity_adapter::{
            handle_opportunity_bid,
            verify_opportunity,
            OpportunityBid,
        },
        state::{
            ChainStore,
            Opportunity,
            OpportunityId,
            OpportunityParams,
            Store,
            UnixTimestampMicros,
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
        signers::Signer,
        types::U256,
    },
    serde::{
        Deserialize,
        Serialize,
    },
    sqlx::types::time::OffsetDateTime,
    std::sync::Arc,
    utoipa::{
        ToResponse,
        ToSchema,
    },
    uuid::Uuid,
};

#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq)]
pub struct EIP712Domain {
    /// The name parameter for the EIP712 domain.
    #[schema(example = "OpportunityAdapter", value_type = Option<String>)]
    pub name:               Option<String>,
    /// The version parameter for the EIP712 domain.
    #[schema(example = "1", value_type = Option<String>)]
    pub version:            Option<String>,
    /// The network chain id parameter for EIP712 domain.
    #[schema(example = "31337", value_type=Option<String>)]
    #[serde(with = "crate::serde::nullable_u256")]
    pub chain_id:           Option<U256>,
    /// The verifying contract address parameter for the EIP712 domain.
    #[schema(example = "0xcA11bde05977b3631167028862bE2a173976CA11", value_type = String)]
    pub verifying_contract: Option<ethers::abi::Address>,
}

/// Similar to OpportunityParams, but with the opportunity id included.
#[derive(Serialize, Deserialize, ToSchema, Clone, ToResponse)]
pub struct OpportunityParamsWithMetadata {
    /// The opportunity unique id
    #[schema(example = "obo3ee3e-58cc-4372-a567-0e02b2c3d479", value_type = String)]
    opportunity_id: OpportunityId,
    /// Creation time of the opportunity (in microseconds since the Unix epoch)
    #[schema(example = 1_700_000_000_000_000i128, value_type = i128)]
    creation_time:  UnixTimestampMicros,
    /// opportunity data
    #[serde(flatten)]
    // expands params into component fields in the generated client schemas
    #[schema(inline)]
    params:         OpportunityParams,
    /// The data needed to create the EIP712 domain separator
    eip_712_domain: EIP712Domain,
}

impl OpportunityParamsWithMetadata {
    pub fn get_chain_id(&self) -> &ChainId {
        match &self.params {
            OpportunityParams::V1(params) => &params.chain_id,
        }
    }
}

impl OpportunityParamsWithMetadata {
    pub fn from(val: Opportunity, chain_store: &ChainStore) -> Self {
        OpportunityParamsWithMetadata {
            opportunity_id: val.id,
            creation_time:  val.creation_time,
            params:         val.params,
            eip_712_domain: chain_store.eip_712_domain.clone(),
        }
    }
}

/// Submit an opportunity ready to be executed.
///
/// The opportunity will be verified by the server. If the opportunity is valid, it will be stored in the database
/// and will be available for bidding.
#[utoipa::path(post, path = "/v1/opportunities", request_body = OpportunityParams, responses(
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
    let now_odt = OffsetDateTime::now_utc();
    let opportunity = Opportunity {
        id,
        creation_time: now_odt.unix_timestamp_nanos() / 1000 as UnixTimestampMicros,
        params: versioned_params.clone(),
    };

    verify_opportunity(params.clone(), chain_store, store.relayer.address())
        .await
        .map_err(|e| RestError::InvalidOpportunity(e.to_string()))?;

    if store.opportunity_exists(&opportunity).await {
        return Err(RestError::BadParameters(
            "Duplicate opportunity submission".to_string(),
        ));
    }
    store.add_opportunity(opportunity.clone()).await?;

    store
        .ws
        .broadcast_sender
        .send(NewOpportunity(OpportunityParamsWithMetadata::from(
            opportunity.clone(),
            chain_store,
        )))
        .map_err(|e| {
            tracing::error!("Failed to send update: {}", e);
            RestError::TemporarilyUnavailable
        })?;

    {
        let opportunities_map = &store.opportunity_store.opportunities.read().await;
        tracing::debug!("number of permission keys: {}", opportunities_map.len());
        tracing::debug!(
            "number of opportunities for key: {}",
            opportunities_map
                .get(&params.permission_key)
                .map_or(0, |opps| opps.len())
        );
    }

    let opportunity_with_metadata: OpportunityParamsWithMetadata =
        OpportunityParamsWithMetadata::from(opportunity.clone(), chain_store);

    Ok(opportunity_with_metadata.into())
}

/// Fetch opportunities ready for execution or historical opportunities
/// depending on the mode. You need to provide `chain_id` for historical mode.
/// Opportunities are sorted by creation time in ascending order in historical mode.
#[utoipa::path(get, path = "/v1/opportunities", responses(
(status = 200, description = "Array of opportunities ready for bidding", body = Vec < OpportunityParamsWithMetadata >),
(status = 400, response = ErrorBodyResponse),
(status = 404, description = "Chain id was not found", body = ErrorBodyResponse),
),
params(GetOpportunitiesQueryParams))]
pub async fn get_opportunities(
    State(store): State<Arc<Store>>,
    query_params: Query<GetOpportunitiesQueryParams>,
) -> Result<axum::Json<Vec<OpportunityParamsWithMetadata>>, RestError> {
    // make sure the chain id is valid
    if let Some(chain_id) = query_params.chain_id.clone() {
        store
            .chains
            .get(&chain_id)
            .ok_or(RestError::InvalidChainId)?;
    }

    match query_params.mode.clone() {
        OpportunityMode::Live => {
            let opportunities: Vec<OpportunityParamsWithMetadata> = store
                .opportunity_store
                .opportunities
                .read()
                .await
                .iter()
                .filter_map(|(_key, opportunities)| {
                    let opportunity = opportunities
                        .last()
                        .expect("A permission key vector should have at least one opportunity");

                    let OpportunityParams::V1(params) = opportunity.params.clone();
                    if let Some(query_chain_id) = &query_params.chain_id {
                        if params.chain_id != *query_chain_id {
                            return None;
                        }
                    }
                    store.chains.get(&params.chain_id).map(|chain_store| {
                        OpportunityParamsWithMetadata::from(opportunity.clone(), chain_store)
                    })
                })
                .collect();

            Ok(opportunities.into())
        }
        OpportunityMode::Historical => {
            let chain_id = query_params.chain_id.clone().ok_or_else(|| {
                RestError::BadParameters("Chain id is required on historical mode".to_string())
            })?;
            let opps = store
                .get_opportunities_by_permission_key(
                    chain_id,
                    query_params.permission_key.clone(),
                    query_params.from_time,
                )
                .await?;
            Ok(opps.into())
        }
    }
}

/// Bid on opportunity
#[utoipa::path(post, path = "/v1/opportunities/{opportunity_id}/bids", request_body = OpportunityBid,
params(("opportunity_id" = String, description = "Opportunity id to bid on")), responses(
(status = 200, description = "Bid Result", body = BidResult, example = json ! ({"status": "OK"})),
(status = 400, response = ErrorBodyResponse),
(status = 404, description = "Opportunity or chain id was not found", body = ErrorBodyResponse),
),)]
pub async fn opportunity_bid(
    auth: Auth,
    State(store): State<Arc<Store>>,
    Path(opportunity_id): Path<OpportunityId>,
    Json(opportunity_bid): Json<OpportunityBid>,
) -> Result<Json<BidResult>, RestError> {
    process_opportunity_bid(store, opportunity_id, &opportunity_bid, auth).await
}

pub async fn process_opportunity_bid(
    store: Arc<Store>,
    opportunity_id: OpportunityId,
    opportunity_bid: &OpportunityBid,
    auth: Auth,
) -> Result<Json<BidResult>, RestError> {
    match handle_opportunity_bid(
        store,
        opportunity_id,
        opportunity_bid,
        OffsetDateTime::now_utc(),
        auth,
    )
    .await
    {
        Ok(id) => Ok(BidResult {
            status: "OK".to_string(),
            id,
        }
        .into()),
        Err(e) => Err(e),
    }
}
