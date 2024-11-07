use {
    super::bid::{
        Bid,
        BidChainData,
        BidStatus,
        BidTrait,
    },
    crate::kernel::entities::ChainId,
    std::sync::Arc,
    time::OffsetDateTime,
    tokio::sync::Mutex,
    uuid::Uuid,
};

pub type AuctionId = Uuid;
pub type AuctionLock = Arc<Mutex<()>>;

pub struct _Auction<T: BidTrait> {
    pub id:                  AuctionId,
    pub chain_id:            ChainId,
    pub permission_key:      <T::ChainData as BidChainData>::PermissionKey,
    pub creation_time:       OffsetDateTime,
    pub conclusion_time:     Option<OffsetDateTime>,
    pub bid_collection_time: Option<OffsetDateTime>,
    pub submission_time:     Option<OffsetDateTime>,
    pub tx_hash:             Option<<T::StatusType as BidStatus>::TxHash>,

    pub bids: Vec<Bid<T>>,
}
