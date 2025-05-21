use {
    clickhouse::Row,
    serde::Serialize,
    std::time::Duration,
    tokio::sync::mpsc,
};

pub struct ClickhouseInserter<T> {
    pub sender: mpsc::Sender<T>,
}

const CHANNEL_SIZE: usize = 1000;
const DURATION_PERIOD: Duration = Duration::from_secs(1);
const MAX_ROWS: u64 = 100;
const MAX_BYTES: u64 = 1_048_576;

impl<T: Row + Serialize + Send + Sync + 'static> ClickhouseInserter<T> {
    async fn run(client: clickhouse::Client, table_name: String, mut rx: mpsc::Receiver<T>) {
        // NOTE take a look for official example here https://github.com/ClickHouse/clickhouse-rs/blob/main/examples/inserter.rs
        match client.inserter(&table_name) {
            Ok(inserter) => {
                let mut inserter = inserter
                    .with_period(Some(DURATION_PERIOD)) // https://docs.rs/clickhouse/latest/clickhouse/inserter/struct.Inserter.html#method.with_period
                    .with_max_rows(MAX_ROWS)
                    .with_max_bytes(MAX_BYTES);
                loop {
                    tokio::select! {
                        data = rx.recv() => {
                            match data {
                                Some(data) => {
                                    if let Err(err) = inserter.write(&data) {
                                        tracing::error!(error = ?err, "Failed to insert data to clickhouse inserter.");
                                    } else if let Err(err) = inserter.commit().await {
                                        tracing::error!(error = ?err, "Failed to commit the inserter to clickhouse.");
                                    }
                                }
                                None => break,
                            }
                        },
                        _ = tokio::time::sleep(Duration::from_secs(1)) => {
                            if let Err(err) = inserter.commit().await {
                                tracing::error!(error = ?err, "Failed to commit the inserter to clickhouse.");
                            }
                        }
                    }
                }
            }
            Err(err) => {
                tracing::error!(error = ?err, "Failed to create clickhouse inserter.");
            }
        }
    }

    pub fn new(client: clickhouse::Client, table_name: String) -> Self {
        let (tx, rx) = mpsc::channel(CHANNEL_SIZE);
        tokio::spawn(Self::run(client, table_name, rx));
        ClickhouseInserter::<T> { sender: tx }
    }
}
