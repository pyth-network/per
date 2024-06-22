use {
    super::Auth,
    crate::{
        api::{
            bid::BidResult,
            ws::UpdateEvent::NewOpportunity,
            ChainIdQueryParams,
            ErrorBodyResponse,
            RestError,
        },
        config::ChainId,
        opportunity_adapter::{
            handle_opportunity_bid,
            verify_opportunity,
            OpportunityBid,
        },
        state::{
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
        types::{
            Address,
            H256,
            U256,
        },
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
    #[schema(example = "Permit2", value_type = Option<String>)]
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
    pub verifying_contract: Option<Address>,
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
}

impl OpportunityParamsWithMetadata {
    pub fn get_chain_id(&self) -> &ChainId {
        match &self.params {
            OpportunityParams::V1(params) => &params.chain_id,
        }
    }
}

impl OpportunityParamsWithMetadata {
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
        OpportunityParamsWithMetadata::from(opportunity.clone());

    Ok(opportunity_with_metadata.into())
}

/// Fetch all opportunities ready to be exectued.
#[utoipa::path(get, path = "/v1/opportunities", responses(
(status = 200, description = "Array of opportunities ready for bidding", body = Vec < OpportunityParamsWithMetadata >),
(status = 400, response = ErrorBodyResponse),
(status = 404, description = "Chain id was not found", body = ErrorBodyResponse),
),
params(ChainIdQueryParams))]
pub async fn get_opportunities(
    State(store): State<Arc<Store>>,
    query_params: Query<ChainIdQueryParams>,
) -> Result<axum::Json<Vec<OpportunityParamsWithMetadata>>, RestError> {
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
            Some(OpportunityParamsWithMetadata::from(opportunity.clone()))
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

#[derive(Serialize, Deserialize, ToSchema, Clone, ToResponse)]
pub struct OpportunityAdapterConfig {
    /// The chain id as a u64
    #[schema(example = 31337, value_type = u64)]
    pub chain_id:                               u64,
    /// The opportunity factory address
    #[schema(example = "0x0AFA3E194ca60B13a3f455b63Ed16Df044c9AeD4", value_type = String)]
    pub opportunity_adapter_factory:            Address,
    /// The hash of the bytecode used to initialize the opportunity adapter
    #[schema(example = "0x5fd31c9d02e2fc69cc09dfea1a9b726391e0e0862624757c0373f66c3bb8920e", value_type = String)]
    pub opportunity_adapter_init_bytecode_hash: H256,
    /// The permit2 address
    #[schema(example = "0x92ab27f3559c8f18bB86E2b2bBfA15631dE45718", value_type = String)]
    pub permit2:                                Address,
    /// The weth address
    #[schema(example = "0xE1408BbF3076A40C0c30F6E243f0Bc43e4f51850", value_type = String)]
    pub weth:                                   Address,
}

/// Fetch the opportunity adapter config for a chain.
#[utoipa::path(get, path = "/v1/opportunities/{chain_id}/config",
params(("chain_id" = String, description = "Chain id to get opportunity config for")), responses(
(status = 200, description = "The opportunity config for the specified chain ID", body = OpportunityAdapterConfig),
(status = 400, response = ErrorBodyResponse),
(status = 404, description = "Chain id was not found", body = ErrorBodyResponse),
),)]
pub async fn get_opportunity_config(
    State(store): State<Arc<Store>>,
    Path(chain_id): Path<ChainId>,
) -> Result<axum::Json<OpportunityAdapterConfig>, RestError> {
    let chain_store = store
        .chains
        .get(&chain_id)
        .ok_or(RestError::InvalidChainId)?;
    Ok(chain_store.opportunity_adapter_config.clone().into())
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
