use {
    crate::{
        api::{
            ws::UpdateEvent::NewOpportunity,
            ErrorBodyResponse,
            GetOpportunitiesQueryParams,
            OpportunityMode,
            RestError,
        },
        config::ChainId,
        opportunity_adapter::verify_opportunity,
        state::{
            Opportunity,
            OpportunityId,
            OpportunityParams,
            StoreNew,
            UnixTimestampMicros,
        },
    },
    axum::{
        extract::{
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
    sqlx::types::time::OffsetDateTime,
    std::sync::Arc,
    utoipa::{
        ToResponse,
        ToSchema,
    },
    uuid::Uuid,
};


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
}

impl OpportunityParamsWithMetadata {
    pub fn get_chain_id(&self) -> &ChainId {
        match &self.params {
            OpportunityParams::V1(params) => &params.chain_id,
        }
    }
}

impl From<Opportunity> for OpportunityParamsWithMetadata {
    fn from(val: Opportunity) -> Self {
        OpportunityParamsWithMetadata {
            opportunity_id: val.id,
            creation_time:  val.creation_time,
            params:         val.params,
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
    State(store): State<Arc<StoreNew>>,
    Json(versioned_params): Json<OpportunityParams>,
) -> Result<Json<OpportunityParamsWithMetadata>, RestError> {
    let OpportunityParams::V1(params) = versioned_params.clone();
    let chain_store = store
        .store
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

    verify_opportunity(params.clone(), chain_store, store.store.relayer.address())
        .await
        .map_err(|e| {
            tracing::warn!(
                "Failed to verify opportunity: {:?} - params: {:?}",
                e,
                versioned_params
            );
            RestError::InvalidOpportunity(e.to_string())
        })?;

    if store.store.opportunity_exists(&opportunity).await {
        tracing::warn!("Duplicate opportunity submission: {:?}", opportunity);
        return Err(RestError::BadParameters(
            "Duplicate opportunity submission".to_string(),
        ));
    }
    store.store.add_opportunity(opportunity.clone()).await?;

    store
        .store
        .ws
        .broadcast_sender
        .send(NewOpportunity(OpportunityParamsWithMetadata::from(
            opportunity.clone(),
        )))
        .map_err(|e| {
            tracing::error!(
                "Failed to send update: {} - opportunity: {:?}",
                e,
                opportunity
            );
            RestError::TemporarilyUnavailable
        })?;

    {
        let opportunities_map = &store.store.opportunity_store.opportunities.read().await;
        tracing::debug!("number of permission keys: {}", opportunities_map.len());
        tracing::debug!(
            "number of opportunities for key: {}",
            opportunities_map
                .get(&params.permission_key)
                .map_or(0, |opps| opps.len())
        );
    }

    let opportunity_with_metadata: OpportunityParamsWithMetadata = opportunity.into();

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
    State(store): State<Arc<StoreNew>>,
    query_params: Query<GetOpportunitiesQueryParams>,
) -> Result<axum::Json<Vec<OpportunityParamsWithMetadata>>, RestError> {
    // make sure the chain id is valid
    if let Some(chain_id) = query_params.chain_id.clone() {
        store
            .store
            .chains
            .get(&chain_id)
            .ok_or(RestError::InvalidChainId)?;
    }

    match query_params.mode.clone() {
        OpportunityMode::Live => {
            let opportunities: Vec<OpportunityParamsWithMetadata> = store
                .store
                .opportunity_store
                .opportunities
                .read()
                .await
                .iter()
                .map(|(_key, opportunities)| {
                    let opportunity = opportunities
                        .last()
                        .expect("A permission key vector should have at least one opportunity");
                    OpportunityParamsWithMetadata::from(opportunity.clone())
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
        OpportunityMode::Historical => {
            let chain_id = query_params.chain_id.clone().ok_or_else(|| {
                RestError::BadParameters("Chain id is required on historical mode".to_string())
            })?;
            let opps = store
                .store
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
