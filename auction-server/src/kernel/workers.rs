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
    axum_prometheus::metrics,
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
    tracing::instrument,
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


const DELETE_BATCH_SIZE: u64 = 5000;

pub async fn run_delete_pg_db_history(
    db: &PgPool,
    chain_ids: Vec<String>,
    delete_pg_rows_options: DeletePgRowsOptions,
) -> anyhow::Result<()> {
    if delete_pg_rows_options.delete_enabled {
        let delete_interval_secs = delete_pg_rows_options.delete_interval_secs;
        let delete_threshold_secs = delete_pg_rows_options.delete_threshold_secs;

        tracing::info!("Starting delete PG DB history worker, deleting every {} seconds rows that are {} seconds stale...", delete_interval_secs, delete_threshold_secs);
        let mut delete_history_interval =
            tokio::time::interval(Duration::from_secs(delete_interval_secs));

        while !SHOULD_EXIT.load(Ordering::Acquire) {
            tokio::select! {
                _ = delete_history_interval.tick() => {
                    delete_pg_db_bid_history(
                        db,
                        delete_threshold_secs,
                    )
                    .await?;

                    let futures = chain_ids.iter().map(|chain_id| {
                        let db = db.clone();
                        async move {
                            delete_pg_db_opportunity_history(
                                &db,
                                chain_id,
                                delete_threshold_secs,
                            )
                            .await?;
                            Ok::<(), anyhow::Error>(())
                        }
                    });
                    futures::future::try_join_all(futures).await?;
                }
            }
        }
        tracing::info!("Shutting down delete PG DB history worker...");
        Ok(())
    } else {
        tracing::info!("Skipping PG DB history deletion loop...");
        Ok(())
    }
}

#[instrument(
    target = "metrics",
    name = "db_delete_pg_bid_history"
    fields(category = "db_queries", result = "success", name = "delete_pg_bid_history", tracing_enabled),
    skip_all
)]
pub async fn delete_pg_db_bid_history(
    db: &PgPool,
    delete_threshold_secs: u64,
) -> anyhow::Result<()> {
    let threshold = OffsetDateTime::now_utc() - Duration::from_secs(delete_threshold_secs);
    let n_bids_deleted = sqlx::query!(
        "WITH rows_to_delete AS (
            SELECT id FROM bid WHERE creation_time < $1 LIMIT $2
        ) DELETE FROM bid WHERE id IN (SELECT id FROM rows_to_delete)",
        PrimitiveDateTime::new(threshold.date(), threshold.time()),
        DELETE_BATCH_SIZE as i64,
    )
    .execute(db)
    .await
    .map_err(|e| {
        tracing::Span::current().record("result", "error");
        tracing::error!("Failed to delete PG DB bid history: {}", e);
        e
    })?
    .rows_affected();

    metrics::histogram!("db_delete_pg_bid_count").record(n_bids_deleted as f64);

    Ok(())
}

#[instrument(
    target = "metrics",
    name = "db_delete_pg_opportunity_history"
    fields(category = "db_queries", result = "success", name = "delete_pg_opportunity_history", tracing_enabled),
    skip_all
)]
pub async fn delete_pg_db_opportunity_history(
    db: &PgPool,
    chain_id: &str,
    delete_threshold_secs: u64,
) -> anyhow::Result<()> {
    let threshold = OffsetDateTime::now_utc() - Duration::from_secs(delete_threshold_secs);
    // TODO: we filter based on removal_time being not null because some Limo opportunities may be valid longer than the delete threshold.
    // However, this makes it so that we don't delete opportunities that are not removed yet, including unremoved ones due to server restarts
    // and bugs in the code. As this leads to a low memory leak, we should consider a better way to handle this in the future.
    let n_opportunities_deleted = sqlx::query!(
        "WITH rows_to_delete AS (
            SELECT id FROM opportunity WHERE chain_id = $1 AND creation_time < $2 AND removal_time IS NOT NULL LIMIT $3
        ) DELETE FROM opportunity WHERE id IN (SELECT id FROM rows_to_delete)",
        chain_id,
        PrimitiveDateTime::new(threshold.date(), threshold.time()),
        DELETE_BATCH_SIZE as i64,
    )
    .execute(db)
    .await
    .map_err(|e| {
        tracing::Span::current().record("result", "error");
        tracing::error!("Failed to delete PG DB opportunity history: {}", e);
        e
    })?
    .rows_affected();

    metrics::histogram!(
        "db_delete_pg_opportunity_count",
        &[("chain_id", chain_id.to_string())]
    )
    .record(n_opportunities_deleted as f64);

    Ok(())
}
