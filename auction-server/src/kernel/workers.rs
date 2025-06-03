use {
    crate::{
        config::DeletePgRowsOptions,
        kernel::pyth_lazer::{
            PriceFeed,
            PythLazer,
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
    reqwest::Url,
    sqlx::PgPool,
    std::{
        sync::{
            atomic::Ordering,
            Arc,
        },
        time::Duration,
    },
    time::{
        OffsetDateTime,
        PrimitiveDateTime,
    },
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
    let mut receiver = pyth_lazer.subscribe().await?;

    while !SHOULD_EXIT.load(Ordering::Acquire) {
        tokio::select! {
            update = receiver.recv() => {
                let update = update.map_err(|err| {
                    tracing::error!(error = ?err, "Failed to receive price update from Pyth Lazer receiver");
                    err
                })?;
                let updates = update.parsed.clone().price_feeds.into_iter().filter_map(|price| {
                    match pyth_lazer.price_feeds.get(&price.id) {
                        Some(price_feed) => {
                            match price.price.parse::<u64>() {
                                Ok(value) => Some((price_feed.mint, Price {
                                    exponent: price_feed.exponent,
                                    price: value,
                                })),
                                Err(err) => {
                                    tracing::error!(error = ?err, parsed_update = ?update.parsed, "Failed to parse price for lazer message");
                                    None
                                }
                            }
                        }
                        None => {
                            tracing::warn!(id=?price.id, "Lazer price feed not found");
                            None
                        }
                    }
                });
                let mut prices = store.prices.write().await;
                updates.for_each(|(mint, price)| {
                    prices.insert(mint, price);
                });
                drop(prices);
            }
            _ = exit_check_interval.tick() => {}
        }
    }
    tracing::info!("Shutting down price subscription...");
    Ok(())
}


pub async fn run_delete_pg_db_history(
    db: &PgPool,
    delete_pg_rows_options: DeletePgRowsOptions,
) -> anyhow::Result<()> {
    match (
        delete_pg_rows_options.delete_interval_secs,
        delete_pg_rows_options.delete_threshold_secs,
    ) {
        (Some(delete_interval_secs), Some(delete_threshold_secs)) => {
            tracing::info!("Starting delete PG DB history worker, deleting every {} seconds rows that are {} seconds stale...", delete_interval_secs, delete_threshold_secs);
            let mut delete_history_interval =
                tokio::time::interval(Duration::from_secs(delete_interval_secs));

            while !SHOULD_EXIT.load(Ordering::Acquire) {
                tokio::select! {
                    _ = delete_history_interval.tick() => {
                        let threshold = OffsetDateTime::now_utc() - Duration::from_secs(delete_threshold_secs);

                        sqlx::query!(
                            "DELETE FROM opportunity WHERE creation_time < $1",
                            PrimitiveDateTime::new(threshold.date(), threshold.time())
                        )
                        .execute(db)
                        .await?;

                        sqlx::query!(
                            "DELETE FROM bid WHERE creation_time < $1",
                            PrimitiveDateTime::new(threshold.date(), threshold.time())
                        )
                        .execute(db)
                        .await?;
                    }
                }
            }
            tracing::info!("Shutting down delete PG DB history worker...");
            Ok(())
        }
        _ => {
            tracing::info!("Skipping PG DB history deletion loop...");
            Ok(())
        }
    }
}
