use sqlx::{
    Pool,
    Postgres,
};

pub type DB = Pool<Postgres>;
