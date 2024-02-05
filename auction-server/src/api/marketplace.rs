use {
    crate::{
        api::{
            rest::handle_bid,
            RestError,
        },
        config::ChainId,
        liquidation_adapter::{
            make_liquidator_calldata,
            parse_revert_error,
            verify_opportunity,
        },
        state::{
            Store,
            UnixTimestamp,
            VerifiedLiquidationOpportunity,
        },
    },
    axum::{
        extract::State,
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
    utoipa::ToSchema,
    uuid::Uuid,
};

#[derive(Serialize, Deserialize, ToSchema, Clone)]
pub struct TokenQty {
    /// Token contract address
    #[schema(example = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",value_type=String)]
    contract: Address,
    /// Token amount
    #[schema(example = "1000", value_type=String)]
    #[serde(with = "crate::serde::u256")]
    amount:   U256,
}

/// A liquidation opportunity ready to be executed.
/// If a searcher signs the opportunity and have approved enough tokens to liquidation adapter,
/// by calling this contract with the given calldata and structures, they will receive the tokens specified
/// in the receipt_tokens field, and will send the tokens specified in the repay_tokens field.
#[derive(Serialize, Deserialize, ToSchema, Clone)]
pub struct LiquidationOpportunity {
    /// The permission key required for succesful execution of the liquidation.
    #[schema(example = "0xdeadbeefcafe", value_type=String)]
    permission_key: Bytes,
    /// The chain id where the liquidation will be executed.
    #[schema(example = "sepolia", value_type=String)]
    chain_id:       ChainId,
    /// The contract address to call for execution of the liquidation.
    #[schema(example = "0xcA11bde05977b3631167028862bE2a173976CA11", value_type=String)]
    contract:       Address,
    /// Calldata for the contract call.
    #[schema(example = "0xdeadbeef", value_type=String)]
    calldata:       Bytes,
    /// The value to send with the contract call.
    #[schema(example = "1", value_type=String)]
    #[serde(with = "crate::serde::u256")]
    value:          U256,

    repay_tokens:   Vec<TokenQty>,
    receipt_tokens: Vec<TokenQty>,
}

/// Similar to LiquidationOpportunity, but with the opportunity id included.
#[derive(Serialize, Deserialize, ToSchema, Clone)]
pub struct LiquidationOpportunityWithId {
    /// The opportunity unique id
    opportunity_id: Uuid,
    /// opportunity data
    #[serde(flatten)]
    opportunity:    LiquidationOpportunity,
}

impl From<(Address, U256)> for TokenQty {
    fn from(token: (Address, U256)) -> Self {
        TokenQty {
            contract: token.0,
            amount:   token.1,
        }
    }
}

impl From<TokenQty> for (Address, U256) {
    fn from(token: TokenQty) -> Self {
        (token.contract, token.amount)
    }
}

fn parse_tokens(tokens: Vec<TokenQty>) -> Vec<(Address, U256)> {
    tokens.into_iter().map(|token| token.into()).collect()
}

/// Submit a liquidation opportunity ready to be executed.
///
/// The opportunity will be verified by the server. If the opportunity is valid, it will be stored in the database
/// and will be available for bidding.
#[utoipa::path(post, path = "/liquidation/submit_opportunity", request_body = LiquidationOpportunity, responses(
    (status = 200, description = "Opportunity was stored succesfuly with the returned uuid", body = String),
    (status = 400, response=RestError)
),)]
pub async fn submit_opportunity(
    State(store): State<Arc<Store>>,
    Json(opportunity): Json<LiquidationOpportunity>,
) -> Result<String, RestError> {
    let chain_store = store
        .chains
        .get(&opportunity.chain_id)
        .ok_or(RestError::InvalidChainId)?;

    let repay_tokens = parse_tokens(opportunity.repay_tokens);
    let receipt_tokens = parse_tokens(opportunity.receipt_tokens);

    let id = Uuid::new_v4();
    let verified_opportunity = VerifiedLiquidationOpportunity {
        id,
        creation_time: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| RestError::BadParameters("Invalid system time".to_string()))?
            .as_secs() as UnixTimestamp,
        chain_id: opportunity.chain_id.clone(),
        permission_key: opportunity.permission_key.clone(),
        contract: opportunity.contract,
        calldata: opportunity.calldata,
        value: opportunity.value,
        repay_tokens,
        receipt_tokens,
        bidders: Default::default(),
    };

    verify_opportunity(
        verified_opportunity.clone(),
        chain_store,
        store.per_operator.address(),
    )
    .await
    .map_err(|e| RestError::InvalidOpportunity(e.to_string()))?;

    store
        .liquidation_store
        .opportunities
        .write()
        .await
        .insert(opportunity.permission_key.clone(), verified_opportunity);

    Ok(id.to_string())
}

/// Fetch all liquidation opportunities ready to be exectued.
#[utoipa::path(get, path = "/liquidation/fetch_opportunities", responses(
    (status = 200, description = "Array of liquidation opportunities ready for bidding", body = Vec<LiquidationOpportunity>),
    (status = 400, response=RestError)
),)]
pub async fn fetch_opportunities(
    State(store): State<Arc<Store>>,
) -> Result<axum::Json<Vec<LiquidationOpportunityWithId>>, RestError> {
    let opportunities: Vec<LiquidationOpportunityWithId> = store
        .liquidation_store
        .opportunities
        .read()
        .await
        .values()
        .cloned()
        .map(|opportunity| LiquidationOpportunityWithId {
            opportunity_id: opportunity.id,
            opportunity:    LiquidationOpportunity {
                permission_key: opportunity.permission_key,
                chain_id:       opportunity.chain_id,
                contract:       opportunity.contract,
                calldata:       opportunity.calldata,
                value:          opportunity.value,
                repay_tokens:   opportunity
                    .repay_tokens
                    .into_iter()
                    .map(TokenQty::from)
                    .collect(),
                receipt_tokens: opportunity
                    .receipt_tokens
                    .into_iter()
                    .map(TokenQty::from)
                    .collect(),
            },
        })
        .collect();

    Ok(opportunities.into())
}

#[derive(Serialize, Deserialize, ToSchema, Clone)]
pub struct OpportunityBid {
    /// The opportunity id to bid on.
    #[schema(example = "f47ac10b-58cc-4372-a567-0e02b2c3d479",value_type=String)]
    pub opportunity_id: Uuid,
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
#[utoipa::path(post, path = "/liquidation/bid_opportunity", request_body=OpportunityBid, responses(
    (status = 200, description = "Bid Result", body = String),
    (status = 400, response=RestError)
),)]
pub async fn bid_opportunity(
    State(store): State<Arc<Store>>,
    Json(opportunity_bid): Json<OpportunityBid>,
) -> Result<String, RestError> {
    let liquidation = store
        .liquidation_store
        .opportunities
        .read()
        .await
        .get(&opportunity_bid.permission_key)
        .ok_or(RestError::OpportunityNotFound)?
        .clone();


    if liquidation.id != opportunity_bid.opportunity_id {
        return Err(RestError::BadParameters(
            "Invalid opportunity_id".to_string(),
        ));
    }

    // TODO: move this logic to searcher side
    if liquidation.bidders.contains(&opportunity_bid.liquidator) {
        return Err(RestError::BadParameters(
            "Liquidator already bid on this opportunity".to_string(),
        ));
    }

    let chain_store = store
        .chains
        .get(&liquidation.chain_id)
        .ok_or(RestError::InvalidChainId)?;

    let per_calldata = make_liquidator_calldata(
        liquidation.clone(),
        opportunity_bid.clone(),
        chain_store.provider.clone(),
        chain_store.config.adapter_contract,
    )
    .await
    .map_err(|e| RestError::BadParameters(e.to_string()))?;
    match handle_bid(
        store.clone(),
        crate::api::rest::Bid {
            permission_key: liquidation.permission_key.clone(),
            chain_id:       liquidation.chain_id.clone(),
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
            Ok("OK".to_string())
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
