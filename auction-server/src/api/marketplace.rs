use {
    crate::{
        api::{
            rest::handle_bid,
            RestError,
        },
        liquidation_adapter::make_liquidator_calldata,
        state::Store,
    },
    axum::{
        extract::State,
        Json,
    },
    ethers::{
        abi::Address,
        core::types::Signature,
        types::{
            Bytes,
            U256,
        },
    },
    serde::{
        Deserialize,
        Serialize,
    },
    std::sync::Arc,
    utoipa::ToSchema,
    uuid::Uuid,
};

#[derive(Serialize, Deserialize, ToSchema, Clone)]
pub struct TokenQty {
    /// Token contract address
    #[schema(example = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",value_type=String)]
    contract: Address,
    /// Token amount
    #[schema(example = "1000")]
    amount:   String,
}

/// A liquidation opportunity ready to be executed.
/// If a searcher signs the opportunity and have approved enough tokens to liquidation adapter, by calling this contract with the given calldata and structures, they will receive the tokens specified in the receipt_tokens field, and will send the tokens specified in the repay_tokens field.
#[derive(Serialize, Deserialize, ToSchema, Clone)]
pub struct LiquidationOpportunity {
    /// The permission key required for succesful execution of the liquidation.
    #[schema(example = "0xdeadbeefcafe", value_type=String)]
    permission_key: Bytes,
    /// The chain id where the liquidation will be executed.
    #[schema(example = "sepolia")]
    chain_id:       String,
    /// The contract address to call for execution of the liquidation.
    #[schema(example = "0xcA11bde05977b3631167028862bE2a173976CA11", value_type=String)]
    contract:       Address,
    /// Calldata for the contract call.
    #[schema(example = "0xdeadbeef", value_type=String)]
    calldata:       Bytes,

    repay_tokens:   Vec<TokenQty>,
    receipt_tokens: Vec<TokenQty>,
}

impl From<(Address, U256)> for TokenQty {
    fn from(token: (Address, U256)) -> Self {
        TokenQty {
            contract: token.0,
            amount:   token.1.to_string(),
        }
    }
}

impl TryFrom<TokenQty> for (Address, U256) {
    type Error = RestError;

    fn try_from(token: TokenQty) -> Result<Self, Self::Error> {
        let amount = U256::from_dec_str(token.amount.as_str())
            .map_err(|_| RestError::BadParameters("Invalid token amount".to_string()))?;
        Ok((token.contract, amount))
    }
}

fn parse_tokens(tokens: Vec<TokenQty>) -> Result<Vec<(Address, U256)>, RestError> {
    tokens.into_iter().map(|token| token.try_into()).collect()
}

/// Submit a liquidation opportunity ready to be executed.
///
/// The opportunity will be verified by the server. If the opportunity is valid, it will be stored in the database and will be available for bidding.
#[utoipa::path(post, path = "/liquidation/submit_opportunity", request_body = LiquidationOpportunity, responses(
    (status = 200, description = "Opportunity was stored succesfuly", body = String),
    (status = 400, response=RestError)
),)]
pub async fn submit_opportunity(
    State(store): State<Arc<Store>>,
    Json(opportunity): Json<LiquidationOpportunity>,
) -> Result<String, RestError> {
    store
        .chains
        .get(&opportunity.chain_id)
        .ok_or(RestError::InvalidChainId)?;

    let repay_tokens = parse_tokens(opportunity.repay_tokens)?;
    let receipt_tokens = parse_tokens(opportunity.receipt_tokens)?;

    //TODO: Verify if the call actually works

    store.liquidation_store.opportunities.write().await.insert(
        opportunity.permission_key.clone(),
        crate::state::VerifiedLiquidationOpportunity {
            id: Uuid::new_v4(),
            chain: opportunity.chain_id.clone(),
            permission: opportunity.permission_key,
            contract: opportunity.contract,
            calldata: opportunity.calldata,
            repay_tokens,
            receipt_tokens,
        },
    );

    Ok("OK".to_string())
}

/// Fetch all liquidation opportunities ready to be exectued.
#[utoipa::path(get, path = "/liquidation/fetch_opportunities", responses(
    (status = 200, description = "Array of liquidation opportunities ready for bidding", body = Vec<LiquidationOpportunity>),
    (status = 400, response=RestError)
),)]
pub async fn fetch_opportunities(
    State(store): State<Arc<Store>>,
) -> Result<axum::Json<Vec<LiquidationOpportunity>>, RestError> {
    let opportunities: Vec<LiquidationOpportunity> = store
        .liquidation_store
        .opportunities
        .read()
        .await
        .values()
        .cloned()
        .map(|opportunity| LiquidationOpportunity {
            permission_key: opportunity.permission,
            chain_id:       opportunity.chain,
            contract:       opportunity.contract,
            calldata:       opportunity.calldata,
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
        })
        .collect();

    Ok(opportunities.into())
}

#[derive(Serialize, Deserialize, ToSchema, Clone)]
pub struct OpportunityBid {
    /// The opportunity id to bid on.
    #[schema(example = "f47ac10b-58cc-4372-a567-0e02b2c3d479",value_type=String)]
    opportunity_id: Uuid,
    /// The opportunity permission key
    #[schema(example = "0xdeadbeefcafe", value_type=String)]
    permission_key: Bytes,
    /// The bid amount in wei.
    #[schema(example = "1000000000000000000")]
    bid_amount:     String,
    /// How long the bid will be valid for.
    #[schema(example = "1000000000000000000")]
    valid_until:    String,
    /// Liquidator address
    #[schema(example = "0x5FbDB2315678afecb367f032d93F642f64180aa2", value_type=String)]
    liquidator:     Address,
    #[schema(
        example = "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef12"
    ,value_type=String)]
    signature:      Signature,
}

#[derive(Clone, Copy)]
pub struct VerifiedOpportunityBid {
    pub opportunity_id: Uuid,
    pub bid_amount:     U256,
    pub valid_until:    U256,
    pub liquidator:     Address,
    pub signature:      Signature,
}

pub async fn bid_opportunity(
    store: Arc<Store>,
    Json(opportunity_bid): Json<OpportunityBid>,
) -> Result<String, RestError> {
    let opportunities = store.liquidation_store.opportunities.read().await;

    let liquidation = opportunities
        .get(&opportunity_bid.permission_key)
        .ok_or(RestError::OpportunityNotFound)?;

    if liquidation.id != opportunity_bid.opportunity_id {
        return Err(RestError::BadParameters(
            "Invalid opportunity_id".to_string(),
        ));
    }
    let bid_amount = U256::from_dec_str(opportunity_bid.bid_amount.as_str())
        .map_err(|_| RestError::BadParameters("Invalid bid_amount".to_string()))?;
    let valid_until = U256::from_dec_str(opportunity_bid.valid_until.as_str())
        .map_err(|_| RestError::BadParameters("Invalid valid_until".to_string()))?;

    let verified_liquidation_bid = VerifiedOpportunityBid {
        opportunity_id: opportunity_bid.opportunity_id,
        bid_amount,
        valid_until,
        liquidator: opportunity_bid.liquidator,
        signature: opportunity_bid.signature,
    };

    let per_calldata = make_liquidator_calldata(liquidation.clone(), verified_liquidation_bid)
        .map_err(|e| RestError::BadParameters(e.to_string()))?;

    handle_bid(
        store.clone(),
        crate::api::rest::ParsedBid {
            permission_key: liquidation.permission.clone(),
            chain_id:       liquidation.chain.clone(),
            contract:       liquidation.contract,
            calldata:       per_calldata,
            bid_amount:     verified_liquidation_bid.bid_amount,
        },
    )
    .await
}
