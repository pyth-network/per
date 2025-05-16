use sqlx::{
    Pool,
    Postgres,
};

pub type DB = Pool<Postgres>;
pub type DBAnalytics = clickhouse::Client;
