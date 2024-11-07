use {
    super::entities,
    crate::{
        kernel::entities::{
            Evm,
            Svm,
        },
        models::ProfileId,
    },
    ethers::types::{
        Address,
        Bytes,
    },
    serde::{
        de::DeserializeOwned,
        Deserialize,
        Serialize,
    },
    serde_with::serde_as,
    solana_sdk::transaction::VersionedTransaction,
    sqlx::{
        types::{
            BigDecimal,
            Json,
        },
        FromRow,
    },
    std::ops::Deref,
    time::PrimitiveDateTime,
};

#[derive(Clone, Debug, PartialEq, PartialOrd, sqlx::Type)]
#[sqlx(type_name = "chain_type", rename_all = "lowercase")]
pub enum ChainType {
    Evm,
    Svm,
}

#[derive(Clone, FromRow, Debug)]
pub struct Auction {
    pub id:                  entities::AuctionId,
    pub creation_time:       PrimitiveDateTime,
    pub conclusion_time:     Option<PrimitiveDateTime>,
    pub permission_key:      Vec<u8>,
    pub chain_id:            String,
    pub chain_type:          ChainType,
    pub tx_hash:             Option<Vec<u8>>,
    pub bid_collection_time: Option<PrimitiveDateTime>,
    pub submission_time:     Option<PrimitiveDateTime>,
}

#[derive(Clone, Debug, PartialEq, PartialOrd, sqlx::Type)]
#[sqlx(type_name = "bid_status", rename_all = "lowercase")]
pub enum BidStatus {
    Pending,
    Submitted,
    Lost,
    Won,
    Expired,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BundleIndex(pub Option<u32>);
impl Deref for BundleIndex {
    type Target = Option<u32>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BidMetadataEvm {
    pub target_contract: Address,
    pub target_calldata: Bytes,
    pub bundle_index:    BundleIndex,
    pub gas_limit:       u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BidMetadataSvm {
    #[serde(with = "crate::serde::transaction_svm")]
    pub transaction: VersionedTransaction,
}

pub trait BidTrait: entities::BidTrait {
    type Metadata: std::fmt::Debug
        + Clone
        + Serialize
        + DeserializeOwned
        + Send
        + Sync
        + Unpin
        + 'static;

    fn get_chain_type() -> ChainType;
    fn get_bundle_index(bid: &Bid<Self>) -> Option<u32>;
    fn get_bid_entity(bid: Bid<Self>, auction: Option<Auction>) -> entities::Bid<Self>;
}

impl BidTrait for Evm {
    type Metadata = BidMetadataEvm;

    fn get_chain_type() -> ChainType {
        ChainType::Evm
    }

    fn get_bundle_index(bid: &Bid<Self>) -> Option<u32> {
        bid.metadata.bundle_index.0
    }

    fn get_bid_entity(_bid: Bid<Self>, _auction: Option<Auction>) -> entities::Bid<Self> {
        panic!("Not implemented")
    }
}

impl BidTrait for Svm {
    type Metadata = BidMetadataSvm;

    fn get_chain_type() -> ChainType {
        ChainType::Svm
    }

    fn get_bundle_index(_bid: &Bid<Self>) -> Option<u32> {
        None
    }

    fn get_bid_entity(_bid: Bid<Self>, _auction: Option<Auction>) -> entities::Bid<Self> {
        panic!("Not implemented")
    }
}

#[derive(Clone, Debug, FromRow)]
pub struct Bid<T: BidTrait> {
    pub id:              entities::BidId,
    #[allow(dead_code)]
    pub creation_time:   PrimitiveDateTime,
    pub permission_key:  Vec<u8>,
    pub chain_id:        String,
    pub chain_type:      ChainType,
    pub bid_amount:      BigDecimal,
    pub status:          BidStatus,
    pub auction_id:      Option<entities::AuctionId>,
    pub initiation_time: PrimitiveDateTime,
    pub profile_id:      Option<ProfileId>,
    pub metadata:        Json<T::Metadata>,
}

impl<T: BidTrait> Bid<T> {
    pub fn is_for_auction(&self, auction: &Option<Auction>) -> bool {
        match auction {
            Some(a) => self.auction_id == Some(a.id),
            None => self.auction_id.is_none(),
        }
    }

    pub fn get_bundle_index(&self) -> Option<u32> {
        T::get_bundle_index(self)
    }

    pub fn get_bid_entity(&self, auction: Option<Auction>) -> entities::Bid<T> {
        T::get_bid_entity(self.clone(), auction)
    }
}
