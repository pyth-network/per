use {
    clickhouse::Row,
    serde::Serialize,
    std::time::Duration,
    tokio::sync::mpsc,
};

pub struct ClickhouseInserter<T> {
    pub sender: mpsc::Sender<T>,
}

impl<T: Row + Serialize + Send + Sync + 'static> ClickhouseInserter<T> {
    async fn run(client: clickhouse::Client, table_name: String, mut rx: mpsc::Receiver<T>) {
        match client.inserter(&table_name) {
            Ok(inserter) => {
                let mut inserter = inserter
                    .with_period(Some(Duration::from_secs(1)))
                    .with_max_rows(100)
                    .with_max_bytes(1_048_576);
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
        let (tx, rx) = mpsc::channel(1000);
        tokio::spawn(Self::run(client, table_name, rx));
        ClickhouseInserter::<T> { sender: tx }
    }
}
