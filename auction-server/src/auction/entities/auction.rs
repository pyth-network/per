use {
    super::bid::{
        Bid,
        BidChainData,
        BidStatus,
    },
    crate::{
        auction::service::ChainTrait,
        kernel::entities::ChainId,
    },
    std::{
        sync::Arc,
        time::Duration,
    },
    time::OffsetDateTime,
    tokio::sync::Mutex,
    uuid::Uuid,
};

pub type AuctionId = Uuid;
pub type AuctionLock = Arc<Mutex<()>>;

#[derive(Debug, Clone)]
pub struct Auction<T: ChainTrait> {
    pub id:                  AuctionId,
    pub chain_id:            ChainId,
    pub permission_key:      super::PermissionKey<T>,
    pub creation_time:       OffsetDateTime,
    pub conclusion_time:     Option<OffsetDateTime>,
    pub bid_collection_time: OffsetDateTime,
    pub submission_time:     Option<OffsetDateTime>,
    pub tx_hash:             Option<<T::BidStatusType as BidStatus>::TxHash>,

    pub bids: Vec<Bid<T>>,
}

pub enum SubmitType {
    ByServer,
    ByOther,
    Invalid,
}

impl<T: ChainTrait> Auction<T> {
    pub fn try_new(bids: Vec<Bid<T>>, bid_collection_time: OffsetDateTime) -> Option<Self> {
        let bids: Vec<Bid<T>> = bids
            .into_iter()
            .filter(|bid| bid.status.is_pending())
            .collect();
        if bids.is_empty() {
            return None;
        }
        Some(Self {
            id: Uuid::new_v4(),
            chain_id: bids[0].chain_id.clone(),
            permission_key: bids[0].chain_data.get_permission_key(),
            creation_time: OffsetDateTime::now_utc(),
            conclusion_time: None,
            bid_collection_time,
            submission_time: None,
            tx_hash: None,
            bids,
        })
    }

    pub fn is_ready(&self, auction_minimum_lifetime: Duration) -> bool {
        self.bids
            .iter()
            .any(|bid| self.bid_collection_time - bid.initiation_time > auction_minimum_lifetime)
    }
}
