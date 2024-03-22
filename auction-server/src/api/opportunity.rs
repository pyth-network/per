use sqlx::postgres::{PgQueryResult, PgRow};
use sqlx::Row;
use {
    crate::{
        api::{
            bid::BidResult,
            ws::UpdateEvent::NewOpportunity,
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
use sqlx::types::{BigDecimal,time::OffsetDateTime};
use std::str::FromStr;
use ethers::abi::AbiEncode;
use sqlx::types::time::PrimitiveDateTime;
use serde_json::Value::Array;
use ethers::core::utils::hex::hex;

/// Similar to OpportunityParams, but with the opportunity id included.
#[derive(Serialize, Deserialize, ToSchema, Clone, ToResponse)]
pub struct OpportunityParamsWithMetadata {
    /// The opportunity unique id
    #[schema(example = "obo3ee3e-58cc-4372-a567-0e02b2c3d479", value_type=String)]
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
        creation_time: now_odt.unix_timestamp() as UnixTimestamp,
        params: versioned_params.clone(),
        bidders: Default::default(),
    };

    verify_opportunity(params.clone(), chain_store, store.relayer.address())
        .await
        .map_err(|e| RestError::InvalidOpportunity(e.to_string()))?;

    // params.target_contract.
    // let row:PgQueryResult = sqlx::query!("INSERT INTO opportunity (id,
    //                                                         creation_time,
    //                                                         permission_key,
    //                                                         chain_id,
    //                                                         target_contract,
    //                                                         target_call_value,
    //                                                         target_calldata,
    //                                                         sell_tokens,
    //                                                         buy_tokens) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
    //     opportunity.id,
    //     PrimitiveDateTime::new(now_odt.date(), now_odt.time()),
    //     params.permission_key.to_vec(),
    //     params.chain_id,
    //     hex::encode(params.target_contract),
    //     BigDecimal::from_str(&params.target_call_value.to_string()).unwrap(),
    //     params.target_calldata.to_vec(),
    //     serde_json::Value::Array(vec![]),
    //     serde_json::Value::Array(vec![]))
    //     .execute(&store.db)
    //     .await
    //     .map_err(|e| {
    //         tracing::error!("Failed to insert opportunity: {}", e);
    //         RestError::TemporarilyUnavailable
    //     })?;
    // println!("{:?}", row.rows_affected());
    let opportunities_map = &store.opportunity_store.opportunities;
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

/// Fetch all opportunities ready to be exectued.
#[utoipa::path(get, path = "/v1/opportunities", responses(
    (status = 200, description = "Array of opportunities ready for bidding", body = Vec<OpportunityParamsWithMetadata>),
    (status = 400, response = ErrorBodyResponse),
    (status = 404, description = "Chain id was not found", body = ErrorBodyResponse),
),
params(ChainIdQueryParams))]
pub async fn get_opportunities(
    State(store): State<Arc<Store>>,
    query_params: Query<ChainIdQueryParams>,
) -> Result<axum::Json<Vec<OpportunityParamsWithMetadata>>, RestError> {

    // let rows = sqlx::query!("SELECT * FROM opportunity")
    //     .fetch_all(&store.db)
    //     .await
    //     .map_err(|e| {
    //         tracing::error!("Failed to insert opportunity: {}", e);
    //         RestError::TemporarilyUnavailable
    //     })?;
    // println!("{:?}", rows);


    let opportunities: Vec<OpportunityParamsWithMetadata> = store
        .opportunity_store
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

/// Bid on opportunity
#[utoipa::path(post, path = "/v1/opportunities/{opportunity_id}/bids", request_body=OpportunityBid,
    params(("opportunity_id"=String, description = "Opportunity id to bid on")), responses(
    (status = 200, description = "Bid Result", body = BidResult, example = json!({"status": "OK"})),
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
