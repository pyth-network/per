use {
    ethers::types::H256,
    sqlx::types::time::PrimitiveDateTime,
    uuid::Uuid,
};

#[derive(Clone)]
pub struct Auction {
    pub id:                  Uuid,
    pub creation_time:       PrimitiveDateTime,
    pub conclusion_time:     Option<PrimitiveDateTime>,
    pub permission_key:      Vec<u8>,
    pub chain_id:            String,
    pub tx_hash:             Option<H256>,
    pub bid_collection_time: Option<PrimitiveDateTime>,
    pub submission_time:     Option<PrimitiveDateTime>,
}

#[derive(Clone)]
pub struct Profile {
    pub id:            Uuid,
    pub creation_time: Option<PrimitiveDateTime>,
}
