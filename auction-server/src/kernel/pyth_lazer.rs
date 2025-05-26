use {
    crate::state::Price,
    futures::{
        SinkExt,
        StreamExt,
    },
    reqwest::Url,
    serde::{
        Deserialize,
        Serialize,
    },
    serde_with::{
        serde_as,
        DisplayFromStr,
    },
    solana_sdk::pubkey::Pubkey,
    std::collections::HashMap,
    tokio::net::TcpStream,
    tokio_tungstenite::{
        connect_async,
        tungstenite::{
            client::IntoClientRequest,
            Message,
        },
        MaybeTlsStream,
        WebSocketStream,
    },
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SubscribeRequestType {
    Subscribe,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SubscribeRequestProperties {
    Price,
    BestBidPrice,
    BestAskPrice,
    PublisherCount,
    Exponent,
    Confidence,
    FundingRate,
    FundingTimestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SubscribeRequestChannel {
    #[serde(rename = "real_time")]
    RealTime,
    #[serde(rename = "fixed_rate@1ms")]
    FixedRate1Ms,
    #[serde(rename = "fixed_rate@50ms")]
    FixedRate50Ms,
    #[serde(rename = "fixed_rate@200ms")]
    FixedRate200Ms,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SubscribeRequestChain {
    #[serde(rename = "solana")]
    Solana,
    #[serde(rename = "evm")]
    Evm,
    #[serde(rename = "leEcdsa")]
    LeEcdsa,
    #[serde(rename = "leUnsigned")]
    LeUnsigned,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscribeRequest {
    #[serde(rename = "type")]
    pub request_type:   SubscribeRequestType,
    #[serde(rename = "subscriptionId")]
    pub id:             u32,
    #[serde(rename = "priceFeedIds")]
    pub price_feed_ids: Vec<u32>,
    pub properties:     Vec<SubscribeRequestProperties>,
    pub channel:        SubscribeRequestChannel,
    pub chains:         Vec<SubscribeRequestChain>,
}

#[serde_as]
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PriceFeed {
    pub id:       u32,
    #[serde_as(as = "DisplayFromStr")]
    pub mint:     Pubkey,
    pub exponent: i32,
}

pub struct PythLazer {
    url:             Url,
    api_key:         String,
    pub price_feeds: HashMap<u32, PriceFeed>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceFeedSubscriptionUpdate {
    #[serde(rename = "priceFeedId")]
    pub id:    u32,
    pub price: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionUpdateParsed {
    #[serde(rename = "timestampUs")]
    timestamp:       String,
    #[serde(rename = "priceFeeds")]
    pub price_feeds: Vec<PriceFeedSubscriptionUpdate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionUpdate {
    #[serde(rename = "type")]
    update_type: String,
    #[serde(rename = "subscriptionId")]
    id:          u32,
    pub parsed:  SubscriptionUpdateParsed,
}

impl PythLazer {
    pub fn new(url: Url, api_key: String, price_feed_ids: Vec<PriceFeed>) -> Self {
        let mut price_feeds = HashMap::new();
        for price_feed in price_feed_ids {
            price_feeds.insert(price_feed.id, price_feed);
        }
        PythLazer {
            url,
            api_key,
            price_feeds,
        }
    }

    pub async fn subscribe(&self) -> anyhow::Result<WebSocketStream<MaybeTlsStream<TcpStream>>> {
        let mut request = self.url.as_str().into_client_request()?;
        let bearer_token = format!("Bearer {}", self.api_key);
        request
            .headers_mut()
            .insert("Authorization", bearer_token.parse()?);
        let (mut ws_stream, _) = connect_async(request).await?;

        let message = serde_json::to_string(&SubscribeRequest {
            id:             1,
            request_type:   SubscribeRequestType::Subscribe,
            price_feed_ids: self.price_feeds.keys().cloned().collect(),
            properties:     vec![SubscribeRequestProperties::Price],
            channel:        SubscribeRequestChannel::FixedRate200Ms,
            chains:         vec![SubscribeRequestChain::Solana],
        })?;
        println!("Sending message: {}", message);
        ws_stream.send(Message::Text(message)).await?;
        let response = ws_stream.next().await;
        println!("Response: {:?}", response);
        Ok(ws_stream)
    }
}

pub fn calculate_final_amount(
    price: Option<Price>,
    amount: u64,
    decimal: Option<u8>,
) -> Option<f64> {
    match (price.clone(), decimal) {
        (Some(Price { price, exponent }), Some(decimals)) => {
            let unit_divisor = match 10u128.checked_pow(decimals as u32) {
                Some(div) => div,
                None => {
                    tracing::error!(decimal = ?decimal, "Overflow in unit divisor calculation for decimal");
                    return None;
                }
            };
            if exponent > 0 {
                tracing::error!(price = ?price, "Exponent must be negative for lazer price");
                return None;
            }

            let exponent_divisor = match 10u128.checked_pow(-exponent as u32) {
                Some(div) => div,
                None => {
                    tracing::error!(exponent = ?exponent, "Overflow in exponent divisor calculation for lazer price");
                    return None;
                }
            };
            let value = (amount as u128)
                .checked_mul(price as u128)? // scale up price
                .checked_div(unit_divisor)?
                .checked_div(exponent_divisor)?;

            let value_f64 = value as f64; // scale back down to original price
            if value_f64.is_infinite() {
                tracing::error!(value_f64 = ?value_f64, "Final amount is too large to convert to f64");
                return None;
            }

            Some(value_f64)
        }
        _ => {
            tracing::warn!(price = ?price, decimal = ?decimal, "Missing price or decimals for token mint");
            None
        }
    }
}
