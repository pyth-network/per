use sqlx::{
    Pool,
    Postgres,
};

pub type DB = Pool<Postgres>;

#[cfg(test)]
pub type DBAnalytics = clickhouse::Client;
