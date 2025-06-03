use {
    crate::{
        config::DeletePgRowsFlags,
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


const DELETE_BATCH_SIZE: u64 = 1000;

pub async fn run_delete_pg_db_history(
    db: &PgPool,
    delete_pg_rows_flags: Option<DeletePgRowsFlags>,
) -> anyhow::Result<()> {
    match delete_pg_rows_flags {
        Some(DeletePgRowsFlags {
            delete_interval_secs,
            delete_threshold_secs,
        }) => {
            tracing::info!("Starting delete PG DB history worker, deleting every {} seconds rows that are {} seconds stale...", delete_interval_secs, delete_threshold_secs);
            let mut delete_history_interval =
                tokio::time::interval(Duration::from_secs(delete_interval_secs));

            while !SHOULD_EXIT.load(Ordering::Acquire) {
                tokio::select! {
                    _ = delete_history_interval.tick() => {
                        let mut n_bids_deleted: Option<u64> = None;
                        let threshold_bid = OffsetDateTime::now_utc() - Duration::from_secs(delete_threshold_secs);
                        while n_bids_deleted.unwrap_or(DELETE_BATCH_SIZE) >= DELETE_BATCH_SIZE {
                            n_bids_deleted = Some(sqlx::query!(
                                "WITH rows_to_delete AS (
                                    SELECT id FROM bid WHERE creation_time < $1 LIMIT $2
                                ) DELETE FROM bid WHERE id IN (SELECT id FROM rows_to_delete)",
                                 PrimitiveDateTime::new(threshold_bid.date(), threshold_bid.time()),
                                DELETE_BATCH_SIZE as i64,
                            )
                            .execute(db)
                            .await?
                            .rows_affected());
                        }

                        let mut n_opportunities_deleted: Option<u64> = None;
                        let threshold_opportunity = OffsetDateTime::now_utc() - Duration::from_secs(delete_threshold_secs);
                        while n_opportunities_deleted.unwrap_or(DELETE_BATCH_SIZE) >= DELETE_BATCH_SIZE {
                            n_opportunities_deleted = Some(sqlx::query!(
                                "WITH rows_to_delete AS (
                                    SELECT id FROM opportunity WHERE creation_time < $1 LIMIT $2
                                ) DELETE FROM opportunity WHERE id IN (SELECT id FROM rows_to_delete)",
                                PrimitiveDateTime::new(threshold_opportunity.date(), threshold_opportunity.time()),
                                DELETE_BATCH_SIZE as i64,
                            )
                            .execute(db)
                            .await?
                            .rows_affected());
                        }
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
