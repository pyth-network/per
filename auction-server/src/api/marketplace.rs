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
    std::sync::Arc,
    utoipa::ToSchema,
    uuid::Uuid,
};

#[derive(Serialize, Deserialize, ToSchema, Clone)]
pub struct TokenAmount {
    /// Token contract address
    #[schema(example = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",value_type=String)]
    contract: Address,
    /// Token amount
    #[schema(example = "1000")]
    amount:   String,
}

/// An order ready to be fulfilled.
/// If a searcher signs the order and have approved enough tokens to liquidation adapter, by calling this contract with the given calldata and order structures, they will receive the tokens specified in the receipt_tokens field, and will send the tokens specified in the repay_tokens field.
#[derive(Serialize, Deserialize, ToSchema, Clone)]
pub struct Order {
    /// The permission key required for succesful execution of the order.
    #[schema(example = "0xdeadbeefcafe", value_type=String)]
    permission_key: Bytes,
    /// The chain id where the order will be executed.
    #[schema(example = "sepolia")]
    chain_id:       String,
    /// The contract address to call for execution of the order.
    #[schema(example = "0xcA11bde05977b3631167028862bE2a173976CA11", value_type=String)]
    contract:       Address,
    /// Calldata for the contract call.
    #[schema(example = "0xdeadbeef", value_type=String)]
    calldata:       Bytes,

    repay_tokens:   Vec<TokenAmount>,
    receipt_tokens: Vec<TokenAmount>,
}

fn parse_tokens(tokens: Vec<TokenAmount>) -> Result<Vec<(Address, U256)>, RestError> {
    tokens
        .iter()
        .map(|token| {
            let amount = U256::from_dec_str(token.amount.as_str())
                .map_err(|_| RestError::BadParameters("Invalid token amount".to_string()))?;
            Ok((token.contract, amount))
        })
        .collect::<Result<Vec<(Address, U256)>, RestError>>()
}

/// Submit an order ready to be fulfilled.
///
/// The order will be verified by the server. If the order is valid, it will be stored in the database and will be available for bidding.
#[utoipa::path(post, path = "/orders/submit_order", request_body = Order, responses(
    (status = 200, description = "Order was stored succesfuly", body = String),
    (status = 400, response=RestError)
),)]
pub async fn submit_order(
    State(store): State<Arc<Store>>,
    Json(order): Json<Order>,
) -> Result<String, RestError> {
    store
        .chains
        .get(&order.chain_id)
        .ok_or(RestError::InvalidChainId)?;

    let repay_tokens = parse_tokens(order.repay_tokens)?;
    let receipt_tokens = parse_tokens(order.receipt_tokens)?;

    //TODO: Verify if the call actually works

    store.liquidation_store.orders.write().await.insert(
        Uuid::new_v4(),
        crate::state::VerifiedOrder {
            chain: order.chain_id.clone(),
            permission: order.permission_key,
            contract: order.contract,
            calldata: order.calldata,
            repay_tokens,
            receipt_tokens,
        },
    );

    Ok("OK".to_string())
}

/// Fetch all orders ready to be fulfilled.
#[utoipa::path(get, path = "/orders/fetch_orders", responses(
    (status = 200, description = "Array of orders ready to fulfilled", body = Vec<Order>),
    (status = 400, response=RestError)
),)]
pub async fn fetch_orders(
    State(store): State<Arc<Store>>,
) -> Result<axum::Json<Vec<Order>>, RestError> {
    let mut orders: Vec<Order> = Vec::new();
    for order in store.liquidation_store.orders.read().await.values() {
        orders.push(Order {
            permission_key: order.permission.clone(),
            chain_id:       order.chain.clone(),
            contract:       order.contract,
            calldata:       order.calldata.clone(),
            repay_tokens:   order
                .repay_tokens
                .iter()
                .map(|(contract, amount)| TokenAmount {
                    contract: contract.clone(),
                    amount:   amount.to_string(),
                })
                .collect(),
            receipt_tokens: order
                .receipt_tokens
                .iter()
                .map(|(contract, amount)| TokenAmount {
                    contract: contract.clone(),
                    amount:   amount.to_string(),
                })
                .collect(),
        });
    }

    Ok(orders.into())
}

#[derive(Serialize, Deserialize, ToSchema, Clone)]
pub struct OrderBid {
    /// The order id to bid on.
    #[schema(example = "f47ac10b-58cc-4372-a567-0e02b2c3d479",value_type=String)]
    order_id:    Uuid,
    /// The bid amount in wei.
    #[schema(example = "1000000000000000000")]
    bid_amount:  String,
    /// How long the order will be valid for.
    #[schema(example = "1000000000000000000")]
    valid_until: String,
    /// Liquidator address
    #[schema(example = "0x5FbDB2315678afecb367f032d93F642f64180aa2", value_type=String)]
    liquidator:  Address,
    #[schema(
        example = "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef12"
    ,value_type=String)]
    signature:   Signature,
}

#[derive(Clone, Copy)]
pub struct VerifiedOrderBid {
    pub order_id:    Uuid,
    pub bid_amount:  U256,
    pub valid_until: U256,
    pub liquidator:  Address,
    pub signature:   Signature,
}

pub async fn bid_order(
    store: Arc<Store>,
    Json(order_bid): Json<OrderBid>,
) -> Result<String, RestError> {
    let orders = store.liquidation_store.orders.read().await;

    let order = orders
        .get(&order_bid.order_id)
        .ok_or(RestError::OrderNotFound)?;
    let bid_amount = U256::from_dec_str(order_bid.bid_amount.as_str())
        .map_err(|_| RestError::BadParameters("Invalid bid_amount".to_string()))?;
    let valid_until = U256::from_dec_str(order_bid.valid_until.as_str())
        .map_err(|_| RestError::BadParameters("Invalid valid_until".to_string()))?;

    let verified_order_bid = VerifiedOrderBid {
        order_id: order_bid.order_id,
        bid_amount,
        valid_until,
        liquidator: order_bid.liquidator,
        signature: order_bid.signature,
    };

    let per_calldata = make_liquidator_calldata(order.clone(), verified_order_bid)
        .map_err(|e| RestError::BadParameters(e.to_string()))?;

    handle_bid(
        store.clone(),
        crate::api::rest::ParsedBid {
            permission_key: order.permission.clone(),
            chain_id:       order.chain.clone(),
            contract:       order.contract,
            calldata:       per_calldata,
            bid_amount:     verified_order_bid.bid_amount,
        },
    )
    .await
}
