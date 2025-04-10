use {
    super::{
        entities::QuoteCreate as QuoteCreateEntity,
        repository::OPPORTUNITY_PAGE_SIZE_CAP,
        service::{
            add_opportunity::AddOpportunityInput,
            get_opportunities::GetOpportunitiesInput,
            get_quote::{
                is_indicative_price_taker,
                GetQuoteInput,
            },
            handle_opportunity_bid::HandleOpportunityBidInput,
            remove_opportunities::RemoveOpportunitiesInput,
        },
    },
    crate::{
        api::{
            Auth,
            RestError,
            WrappedRouter,
        },
        models,
        state::StoreNew,
    },
    axum::{
        extract::{
            Path,
            Query,
            State,
        },
        http::StatusCode,
        Json,
        Router,
    },
    express_relay_api_types::{
        opportunity::{
            GetOpportunitiesQueryParams,
            Opportunity,
            OpportunityBidEvm,
            OpportunityBidResult,
            OpportunityCreate,
            OpportunityDelete,
            OpportunityDeleteSvm,
            OpportunityId,
            ProgramSvm,
            Quote,
            QuoteCreate,
            Route,
        },
        ErrorBodyResponse,
    },
    std::sync::Arc,
    time::OffsetDateTime,
};

fn get_program(auth: &Auth) -> Result<ProgramSvm, RestError> {
    match auth {
        Auth::Authorized(_, profile) => {
            if profile.role == models::ProfileRole::Searcher {
                return Err(RestError::Forbidden);
            }

            match profile.name.as_str() {
                "limo" => Ok(ProgramSvm::Limo),
                _ => Err(RestError::Forbidden),
            }
        }
        Auth::Admin => Err(RestError::Forbidden),
        Auth::Unauthorized => Err(RestError::Unauthorized),
    }
}

/// Bid on opportunity.
#[utoipa::path(post, path = "/v1/opportunities/{opportunity_id}/bids", request_body = OpportunityBidEvm,
params(("opportunity_id" = String, description = "Opportunity id to bid on")), responses(
(status = 200, description = "Bid Result", body = OpportunityBidResult, example = json ! ({"status": "OK"})),
(status = 400, response = ErrorBodyResponse),
(status = 404, description = "Opportunity or chain id was not found", body = ErrorBodyResponse),
),)]
pub async fn opportunity_bid(
    auth: Auth,
    State(store): State<Arc<StoreNew>>,
    Path(opportunity_id): Path<OpportunityId>,
    Json(opportunity_bid): Json<OpportunityBidEvm>,
) -> Result<Json<OpportunityBidResult>, RestError> {
    match store
        .opportunity_service_evm
        .handle_opportunity_bid(HandleOpportunityBidInput {
            opportunity_id,
            opportunity_bid,
            initiation_time: OffsetDateTime::now_utc(),
            auth,
        })
        .await
    {
        Ok(id) => Ok(OpportunityBidResult {
            status: "OK".to_string(),
            id,
        }
        .into()),
        Err(e) => Err(e),
    }
}

/// Submit an opportunity ready to be executed.
///
/// The opportunity will be verified by the server. If the opportunity is valid, it will be stored in the database
/// and will be available for bidding.
#[utoipa::path(post, path = "/v1/opportunities", request_body = OpportunityCreate, responses(
    (status = 200, description = "The created opportunity", body = Opportunity),
    (status = 400, response = ErrorBodyResponse),
    (status = 404, description = "Chain id was not found", body = ErrorBodyResponse),
),)]
pub async fn post_opportunity(
    auth: Auth,
    State(store): State<Arc<StoreNew>>,
    Json(params): Json<OpportunityCreate>,
) -> Result<Json<Opportunity>, RestError> {
    let opportunity_with_metadata: Opportunity = match params {
        OpportunityCreate::Evm(params) => store
            .opportunity_service_evm
            .add_opportunity(AddOpportunityInput {
                opportunity: params.into(),
            })
            .await?
            .into(),
        OpportunityCreate::Svm(params) => {
            if get_program(&auth)? != params.get_program() {
                return Err(RestError::Forbidden);
            }

            store
                .opportunity_service_svm
                .add_opportunity(AddOpportunityInput {
                    opportunity: params.into(),
                })
                .await?
                .into()
        }
    };
    Ok(opportunity_with_metadata.into())
}

/// Fetch opportunities ready for execution or historical opportunities
/// depending on the mode.
///
/// You need to provide `chain_id` for historical mode.
/// Opportunities are sorted by creation time in ascending order.
/// Total number of opportunities returned is capped by the server to preserve bandwidth.
#[utoipa::path(get, path = "/v1/opportunities", responses(
(status = 200, description = "Array of opportunities ready for bidding", body = Vec < Opportunity >),
(status = 400, response = ErrorBodyResponse),
(status = 404, description = "Chain id was not found", body = ErrorBodyResponse),
),
params(GetOpportunitiesQueryParams))]
pub async fn get_opportunities(
    State(store): State<Arc<StoreNew>>,
    query_params: Query<GetOpportunitiesQueryParams>,
) -> Result<axum::Json<Vec<Opportunity>>, RestError> {
    let opportunities_evm = store
        .opportunity_service_evm
        .get_opportunities(GetOpportunitiesInput {
            query_params: query_params.clone().0,
        })
        .await;
    let opportunities_svm = store
        .opportunity_service_svm
        .get_opportunities(GetOpportunitiesInput {
            query_params: query_params.clone().0,
        })
        .await;

    if opportunities_evm == Err(RestError::InvalidChainId)
        && opportunities_svm == Err(RestError::InvalidChainId)
    {
        Err(RestError::InvalidChainId)
    } else {
        let mut opportunities: Vec<Opportunity> = vec![];

        match opportunities_evm {
            Ok(opportunities_evm) => {
                opportunities.extend(
                    opportunities_evm
                        .into_iter()
                        .map(|o| o.into())
                        .collect::<Vec<Opportunity>>(),
                );
            }
            Err(RestError::InvalidChainId) => {}
            Err(error) => {
                return Err(error);
            }
        }

        match opportunities_svm {
            Ok(opportunities_svm) => {
                opportunities.extend(
                    opportunities_svm
                        .into_iter()
                        .map(|o| o.into())
                        .collect::<Vec<Opportunity>>(),
                );
            }
            Err(RestError::InvalidChainId) => {}
            Err(error) => {
                return Err(error);
            }
        }

        opportunities.sort_by_key(|a| a.creation_time());
        Ok(Json(
            opportunities
                .into_iter()
                .take(std::cmp::min(query_params.limit, OPPORTUNITY_PAGE_SIZE_CAP))
                .collect(),
        ))
    }
}

const MEMO_MAX_LENGTH: usize = 100;

/// Submit a quote request.
///
/// The server will create an opportunity and receive searcher bids
/// After a certain time, the winning bid will be returned as the response.
#[utoipa::path(post, path = "/v1/opportunities/quote", request_body = QuoteCreate, responses(
    (status = 200, description = "The created quote", body = Quote),
    (status = 400, response = ErrorBodyResponse),
    (status = 404, description = "No quote available right now", body = ErrorBodyResponse),
),)]
pub async fn post_quote(
    State(store): State<Arc<StoreNew>>,
    Json(params): Json<QuoteCreate>,
) -> Result<Json<Quote>, RestError> {
    if let Some(address) = params.get_user_wallet_address() {
        if is_indicative_price_taker(&address) {
            return Err(RestError::BadParameters(
                "Invalid user wallet address".to_string(),
            ));
        }
    }

    if let Some(length) = params.get_memo_length() {
        if length > MEMO_MAX_LENGTH {
            return Err(RestError::BadParameters(format!(
                "Memo must be less than {} characters",
                MEMO_MAX_LENGTH
            )));
        }
    }

    let quote_create: QuoteCreateEntity = params.into();

    let quote = store
        .opportunity_service_svm
        .get_quote(GetQuoteInput { quote_create })
        .await?;

    Ok(Json(quote.into()))
}

/// Delete all opportunities for specified data.
#[utoipa::path(delete, path = "/v1/opportunities", request_body = OpportunityDelete,
security(
    ("bearerAuth" = []),
),
responses(
(status = 204, description = "Opportunities deleted successfully"),
(status = 400, response = ErrorBodyResponse),
(status = 404, description = "Chain id was not found", body = ErrorBodyResponse),
),)]
pub async fn delete_opportunities(
    auth: Auth,
    State(store): State<Arc<StoreNew>>,
    Json(opportunity_delete): Json<OpportunityDelete>,
) -> Result<StatusCode, RestError> {
    match opportunity_delete {
        OpportunityDelete::Evm(_) => Err(RestError::Forbidden),
        OpportunityDelete::Svm(params_svm) => {
            let OpportunityDeleteSvm::V1(params) = params_svm;
            if get_program(&auth)? != params.program {
                return Err(RestError::Forbidden);
            }

            store
                .opportunity_service_svm
                .remove_opportunities(RemoveOpportunitiesInput {
                    chain_id:           params.chain_id,
                    permission_account: params.permission_account,
                    router:             params.router,
                })
                .await?;

            Ok(StatusCode::NO_CONTENT)
        }
    }
}

pub fn get_routes(store: Arc<StoreNew>) -> Router<Arc<StoreNew>> {
    WrappedRouter::new(store)
        .route(Route::PostOpportunity, post_opportunity)
        .route(Route::PostQuote, post_quote)
        .route(Route::OpportunityBid, opportunity_bid)
        .route(Route::GetOpportunities, get_opportunities)
        .route(Route::DeleteOpportunities, delete_opportunities)
        .router
}
