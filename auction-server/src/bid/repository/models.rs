use {
    super::entities,
    crate::{
        kernel::entities::{
            Evm,
            PermissionKeySvm,
            Svm,
        },
        models::ProfileId,
    },
    ethers::types::{
        Address,
        Bytes,
        H256,
        U256,
    },
    serde::{
        de::DeserializeOwned,
        Deserialize,
        Serialize,
    },
    serde_json::from_slice,
    serde_with::serde_as,
    solana_sdk::{
        signature::Signature,
        transaction::VersionedTransaction,
    },
    sqlx::{
        types::{
            BigDecimal,
            Json,
        },
        FromRow,
    },
    std::{
        num::ParseIntError,
        ops::Deref,
    },
    time::{
        PrimitiveDateTime,
        UtcOffset,
    },
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
    fn get_transaction_hash(
        auction: Option<Auction>,
    ) -> anyhow::Result<Option<<Self::StatusType as entities::BidStatus>::TxHash>>;
    fn get_bid_amount(bid: &Bid<Self>) -> anyhow::Result<Self::BidAmount>;
    fn get_bid_status(
        bid: &Bid<Self>,
        auction: Option<Auction>,
    ) -> anyhow::Result<Self::StatusType>;
    fn get_chain_data(bid: &Bid<Self>) -> anyhow::Result<Self::ChainData>;
}

impl BidTrait for Evm {
    type Metadata = BidMetadataEvm;

    fn get_chain_type() -> ChainType {
        ChainType::Evm
    }

    fn get_transaction_hash(
        auction: Option<Auction>,
    ) -> anyhow::Result<Option<<Self::StatusType as entities::BidStatus>::TxHash>> {
        if let Some(auction) = auction {
            if let Some(tx_hash) = auction.tx_hash {
                let slice: [u8; 32] = tx_hash.try_into().map_err(|e| {
                    anyhow::anyhow!("Failed to convert evm transaction hash to slice {:?}", e)
                })?;
                return Ok(Some(H256::from(slice)));
            }
        }
        Ok(None)
    }

    fn get_bundle_index(bid: &Bid<Self>) -> Option<u32> {
        bid.metadata.bundle_index.0
    }

    fn get_bid_amount(bid: &Bid<Self>) -> anyhow::Result<Self::BidAmount> {
        Self::BidAmount::from_dec_str(bid.bid_amount.to_string().as_str())
            .map_err(|e| anyhow::anyhow!(e))
    }

    fn get_bid_status(
        bid: &Bid<Self>,
        auction: Option<Auction>,
    ) -> anyhow::Result<Self::StatusType> {
        let tx_hash = Self::get_transaction_hash(auction)?;
        let index = Self::get_bundle_index(bid);
        match bid.status {
            BidStatus::Pending => Ok(entities::BidStatusEvm::Pending),
            BidStatus::Submitted => {
                if tx_hash.is_none() || index.is_none() {
                    return Err(anyhow::anyhow!(
                        "Submitted bid should have a tx_hash and index"
                    ));
                }
                Ok(entities::BidStatusEvm::Submitted {
                    tx_hash: tx_hash.expect("Failed to extract tx_hash from 'Some' value"),
                    index:   index.expect("Failed to extract index from 'Some' value"),
                })
            }
            BidStatus::Won => {
                if tx_hash.is_none() || index.is_none() {
                    return Err(anyhow::anyhow!("Won bid should have a tx_hash and index"));
                }
                Ok(entities::BidStatusEvm::Won {
                    tx_hash: tx_hash.expect("Failed to extract tx_hash from 'Some' value"),
                    index:   index.expect("Failed to extract index from 'Some' value"),
                })
            }
            BidStatus::Lost => Ok(entities::BidStatusEvm::Lost { tx_hash, index }),
            BidStatus::Expired => Err(anyhow::anyhow!("Evm bid cannot be expired")),
        }
    }

    fn get_chain_data(bid: &Bid<Self>) -> anyhow::Result<Self::ChainData> {
        Ok(Self::ChainData {
            target_contract: bid.metadata.target_contract,
            target_calldata: bid.metadata.target_calldata.clone(),
            gas_limit:       U256::from(bid.metadata.gas_limit),
            permission_key:  Bytes::from(bid.permission_key.clone()),
        })
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

    fn get_bid_amount(bid: &Bid<Self>) -> anyhow::Result<Self::BidAmount> {
        bid.bid_amount
            .to_string()
            .parse()
            .map_err(|e: ParseIntError| anyhow::anyhow!(e))
    }

    fn get_transaction_hash(
        auction: Option<Auction>,
    ) -> anyhow::Result<Option<<Self::StatusType as entities::BidStatus>::TxHash>> {
        if let Some(auction) = auction {
            if let Some(tx_hash) = auction.tx_hash {
                let slice: [u8; 64] = tx_hash.try_into().map_err(|e| {
                    anyhow::anyhow!("Failed to convert svm transaction hash to slice {:?}", e)
                })?;
                return Ok(Some(Signature::from(slice)));
            }
        }
        Ok(None)
    }

    fn get_bid_status(
        bid: &Bid<Self>,
        auction: Option<Auction>,
    ) -> anyhow::Result<Self::StatusType> {
        let signature = Self::get_transaction_hash(auction)?;
        match bid.status {
            BidStatus::Pending => Ok(entities::BidStatusSvm::Pending),
            BidStatus::Submitted => match signature {
                Some(signature) => Ok(entities::BidStatusSvm::Submitted { signature }),
                None => Err(anyhow::anyhow!("Submitted bid should have a result")),
            },
            BidStatus::Won => match signature {
                Some(signature) => Ok(entities::BidStatusSvm::Won { signature }),
                None => Err(anyhow::anyhow!("Won bid should have a result")),
            },
            BidStatus::Lost => Ok(entities::BidStatusSvm::Lost { signature }),
            BidStatus::Expired => match signature {
                Some(signature) => Ok(entities::BidStatusSvm::Expired { signature }),
                None => Err(anyhow::anyhow!("Expired bid should have a result")),
            },
        }
    }

    fn get_chain_data(bid: &Bid<Self>) -> anyhow::Result<Self::ChainData> {
        let slice: [u8; 64] =
            bid.permission_key.clone().try_into().map_err(|e| {
                anyhow::anyhow!("Failed to convert permission key to slice {:?}", e)
            })?;
        let permission_key: PermissionKeySvm = PermissionKeySvm(slice);
        Ok(Self::ChainData {
            transaction:        bid.metadata.transaction.clone(),
            router:             entities::BidChainDataSvm::get_router(&permission_key),
            permission_account: entities::BidChainDataSvm::get_permission_account(&permission_key),
        })
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

    pub fn get_bid_entity(&self, auction: Option<Auction>) -> anyhow::Result<entities::Bid<T>> {
        Ok(entities::Bid {
            id:              self.id,
            chain_id:        self.chain_id.clone(),
            initiation_time: self.initiation_time.assume_offset(UtcOffset::UTC),
            profile_id:      self.profile_id,

            amount:     T::get_bid_amount(self)?,
            status:     T::get_bid_status(self, auction)?,
            chain_data: T::get_chain_data(self)?,
        })
    }
}
