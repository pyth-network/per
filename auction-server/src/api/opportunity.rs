use {
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
            ChainStore,
            Opportunity,
            OpportunityId,
            OpportunityParams,
            Store,
            UnixTimestampMicros,
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
    serde::{Deserialize, Serialize},
    sqlx::types::time::OffsetDateTime,
    std::sync::Arc,
    utoipa::{
        ToResponse,
        ToSchema,
    },
    uuid::Uuid,
};

#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq)]
pub struct OpportunityAdapterSignatureConfig {
    /// The raw type string for the opportunity
    #[schema(example = "Opportunity(TokenAmount sellTokens,TokenAmount buyTokens,address targetContract,bytes targetCalldata,uint256 targetCallValue,uint256 bidAmount,uint256 validUntil)TokenAmount(address token,uint256 amount)", value_type = String)]
    pub opportunity_type: String,
    /// The domain name parameter for the EIP712 domain separator.
    #[schema(example = "OpportunityAdapter", value_type = String)]
    pub domain_name: String,
    /// The domain version parameter for the EIP712 domain separator.
    #[schema(example = "1", value_type = String)]
    pub domain_version: String,
    /// The network chain id of the opportunity adapter contract
    #[schema(example = 1, value_type = u64)]
    pub chain_network_id: u64,
    /// The opportunity adapter contract address
    #[schema(example = "0xcA11bde05977b3631167028862bE2a173976CA11", value_type = String)]
    pub contract_address: ethers::abi::Address,
}

/// Similar to OpportunityParams, but with the opportunity id included.
#[derive(Serialize, Deserialize, ToSchema, Clone, ToResponse)]
pub struct OpportunityParamsWithMetadata {
    /// The opportunity unique id
    #[schema(example = "obo3ee3e-58cc-4372-a567-0e02b2c3d479", value_type = String)]
    opportunity_id: OpportunityId,
    /// Creation time of the opportunity (in microseconds since the Unix epoch)
    #[schema(example = 1_700_000_000_000_000i128, value_type = i128)]
    creation_time: UnixTimestampMicros,
    /// opportunity data
    #[serde(flatten)]
    // expands params into component fields in the generated client schemas
    #[schema(inline)]
    params: OpportunityParams,
    signature_config: OpportunityAdapterSignatureConfig,
}

impl OpportunityParamsWithMetadata {
    pub fn get_chain_id(&self) -> &ChainId {
        match &self.params {
            OpportunityParams::V1(params) => &params.chain_id,
        }
    }
}

impl OpportunityAdapterSignatureConfig {
    pub fn from(val: &ChainStore) -> Self {
        OpportunityAdapterSignatureConfig {
            domain_name: val.signature_config.opportunity_adapter.domain_name.clone(),
            domain_version: val
                .signature_config
                .opportunity_adapter
                .domain_version
                .clone(),
            contract_address: val.config.opportunity_adapter_contract,
            chain_network_id: val.network_id,
            opportunity_type: val
                .signature_config
                .opportunity_adapter
                .opportunity_type
                .clone(),
        }
    }
}

impl OpportunityParamsWithMetadata {
    fn from(val: Opportunity, chain_store: &ChainStore) -> Self {
        OpportunityParamsWithMetadata {
            opportunity_id: val.id,
            creation_time: val.creation_time,
            params: val.params,
            signature_config: OpportunityAdapterSignatureConfig::from(chain_store),
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
        .map(|(_key, opportunities)| {
            opportunities
                .last()
                .expect("A permission key vector should have at least one opportunity")
                .clone()
        })
        .filter_map(|opportunity| {
            let OpportunityParams::V1(params) = &opportunity.params;
            match store.chains.get(&params.chain_id) {
                Some(_chain_store) => {
                    if query_params.chain_id.is_some() {
                        let query_chain_id = query_params.chain_id.clone().unwrap();
                        if params.chain_id != query_chain_id {
                            return None;
                        }
                    }
                    Some(OpportunityParamsWithMetadata::from(
                        opportunity.clone(),
                        _chain_store,
                    ))
                }
                None => None,
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
    State(store): State<Arc<Store>>,
    Path(opportunity_id): Path<OpportunityId>,
    Json(opportunity_bid): Json<OpportunityBid>,
) -> Result<Json<BidResult>, RestError> {
    process_opportunity_bid(store, opportunity_id, &opportunity_bid).await
}

pub async fn process_opportunity_bid(
    store: Arc<Store>,
    opportunity_id: OpportunityId,
    opportunity_bid: &OpportunityBid,
) -> Result<Json<BidResult>, RestError> {
    match handle_opportunity_bid(store, opportunity_id, opportunity_bid).await {
        Ok(id) => Ok(BidResult {
            status: "OK".to_string(),
            id,
        }
        .into()),
        Err(e) => Err(e),
    }
}
