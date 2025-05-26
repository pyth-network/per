use {
    crate::{
        kernel::pyth_lazer::{
            PriceFeed,
            PythLazer,
            SubscriptionUpdate,
        },
        server::{
            EXIT_CHECK_INTERVAL,
            SHOULD_EXIT,
        },
        state::{
            Price,
            Store,
        },
    },
    futures::SinkExt,
    reqwest::Url,
    solana_sdk::pubkey::Pubkey,
    std::{
        collections::HashMap,
        sync::{
            atomic::Ordering,
            Arc,
        },
    },
    tokio_stream::StreamExt,
    tokio_tungstenite::tungstenite::Message,
};


pub async fn run_price_subscription(
    store: Arc<Store>,
    url: String,
    api_key: String,
    price_feeds: Vec<PriceFeed>,
) -> anyhow::Result<()> {
    tracing::info!("Starting price subcription...");
    let mut exit_check_interval = tokio::time::interval(EXIT_CHECK_INTERVAL);

    let pyth_lazer = PythLazer::new(Url::parse(&url)?, api_key, price_feeds);
    let mut stream = pyth_lazer.subscribe().await?;

    while !SHOULD_EXIT.load(Ordering::Acquire) {
        tokio::select! {
            update = stream.next() => {
                let message = match update {
                    Some(message) => {
                        match message {
                            Ok(message) => message,
                            Err(err) => {
                                tracing::error!(error = ?err, "Error receiving message from pyth lazer");
                                continue;
                            }
                        }
                    }
                    None => {
                        tracing::error!("Pyth lazer stream closed");
                        break;
                    },
                };

                let message: SubscriptionUpdate = match message {
                    Message::Text(text) => {
                        let response: Result<SubscriptionUpdate, serde_json::Error> = serde_json::from_str(&text);
                        match response {
                            Ok(response) => response,
                            Err(_) => continue,
                        }
                    }
                    Message::Binary(binary) => {
                        let response: Result<SubscriptionUpdate, serde_json::Error> = serde_json::from_slice(binary.as_slice());
                        match response {
                            Ok(response) => response,
                            Err(_) => continue,
                        }
                    }
                    Message::Close(_) => {
                        tracing::error!("Pyth lazer stream closed");
                        break;
                    }
                    Message::Pong(_) => continue,
                    Message::Ping(data) => {
                        let _ = stream.send(Message::Pong(data)).await;
                        continue;
                    },
                    Message::Frame(_) => continue,
                };

                let mut updates = HashMap::<Pubkey, Price>::new();
                let parsed_update = message.parsed;
                for price in parsed_update.price_feeds.clone() {
                    match pyth_lazer.price_feeds.get(&price.id) {
                        Some(price_feed) => {
                            let parsed_value = match price.price.parse::<u64>() {
                                Ok(value) => value,
                                Err(err) => {
                                    tracing::error!(error = ?err, parsed_update = ?parsed_update, "Failed to parse price for lazer message");
                                    continue;
                                }
                            };
                            updates.insert(price_feed.mint, Price {
                                exponent: price_feed.exponent,
                                price: parsed_value,
                            });
                        }
                        None => {
                            tracing::warn!(id=?price.id, "Lazer price feed not found");
                        }
                    }
                }

                let mut prices = store.prices.write().await;
                for (mint, price) in updates.iter() {
                    prices.insert(*mint, price.clone());
                }
                drop(prices);
            }
            _ = exit_check_interval.tick() => {}
        }
    }
    tracing::info!("Shutting down transaction submitter...");
    Ok(())
}
