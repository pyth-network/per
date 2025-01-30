use {
    super::entities::{
        self,
        BidChainData,
    },
    crate::{
        auction::service::ChainTrait,
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
    serde_json::json,
    solana_sdk::{
        signature::Signature,
        transaction::VersionedTransaction,
    },
    sqlx::{
        postgres::PgArguments,
        query::Query,
        types::{
            BigDecimal,
            Json,
        },
        FromRow,
        Postgres,
    },
    std::{
        fmt::Debug,
        num::ParseIntError,
        ops::Deref,
        str::FromStr,
    },
    time::{
        OffsetDateTime,
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
#[allow(dead_code)]
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
#[sqlx(type_name = "bid_status", rename_all = "snake_case")]
pub enum BidStatus {
    Pending,
    AwaitingSignature,
    Submitted,
    Lost,
    Won,
    Failed,
    Expired,
    Cancelled,
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
    #[serde(with = "express_relay_api_types::serde::transaction_svm")]
    pub transaction: VersionedTransaction,
}

pub trait ModelTrait<T: ChainTrait> {
    type BidMetadataType: Debug
        + Clone
        + Serialize
        + DeserializeOwned
        + Send
        + Sync
        + Unpin
        + 'static;

    fn get_chain_type() -> ChainType;
    fn get_bid_bundle_index(bid: &Bid<T>) -> Option<u32>;
    fn get_bid_status_auction_entity(
        auction: Option<Auction>,
    ) -> anyhow::Result<Option<entities::BidStatusAuction<T::BidStatusType>>>;
    fn get_bid_amount_entity(bid: &Bid<T>) -> anyhow::Result<T::BidAmountType>;
    fn get_bid_status_entity(
        bid: &Bid<T>,
        auction: Option<Auction>,
    ) -> anyhow::Result<T::BidStatusType>;

    fn convert_bid_status(status: &T::BidStatusType) -> BidStatus;
    fn get_chain_data_entity(bid: &Bid<T>) -> anyhow::Result<T::BidChainDataType>;

    fn convert_permission_key(permission_key: &entities::PermissionKey<T>) -> Vec<u8>;
    fn convert_amount(amount: &T::BidAmountType) -> BigDecimal;

    fn get_metadata(chain_data: &T::BidChainDataType) -> Self::BidMetadataType;
    fn get_update_bid_query(
        bid: &entities::Bid<T>,
        new_status: T::BidStatusType,
    ) -> anyhow::Result<Query<'_, Postgres, PgArguments>>;
}

impl ModelTrait<Evm> for Evm {
    type BidMetadataType = BidMetadataEvm;

    fn get_chain_type() -> ChainType {
        ChainType::Evm
    }

    fn get_bid_status_auction_entity(
        auction: Option<Auction>,
    ) -> anyhow::Result<Option<entities::BidStatusAuction<entities::BidStatusEvm>>> {
        if let Some(auction) = auction {
            if let Some(tx_hash) = auction.tx_hash {
                let slice: [u8; 32] = tx_hash.try_into().map_err(|e| {
                    anyhow::anyhow!("Failed to convert evm transaction hash to slice {:?}", e)
                })?;
                return Ok(Some(entities::BidStatusAuction {
                    tx_hash: H256::from(slice),
                    id:      auction.id,
                }));
            }
        }
        Ok(None)
    }

    fn get_bid_bundle_index(bid: &Bid<Evm>) -> Option<u32> {
        bid.metadata.bundle_index.0
    }

    fn get_bid_amount_entity(bid: &Bid<Evm>) -> anyhow::Result<entities::BidAmountEvm> {
        entities::BidAmountEvm::from_dec_str(bid.bid_amount.to_string().as_str())
            .map_err(|e| anyhow::anyhow!(e))
    }

    fn get_bid_status_entity(
        bid: &Bid<Evm>,
        auction: Option<Auction>,
    ) -> anyhow::Result<entities::BidStatusEvm> {
        let bid_status_auction = Self::get_bid_status_auction_entity(auction)?;
        let index = Self::get_bid_bundle_index(bid);
        match bid.status {
            BidStatus::Pending => Ok(entities::BidStatusEvm::Pending),
            BidStatus::AwaitingSignature => {
                Err(anyhow::anyhow!("Evm bid cannot be awaiting signature"))
            }
            BidStatus::Submitted => {
                if bid_status_auction.is_none() || index.is_none() {
                    return Err(anyhow::anyhow!(
                        "Submitted bid should have a tx_hash, auction_id and index"
                    ));
                }
                Ok(entities::BidStatusEvm::Submitted {
                    auction: bid_status_auction
                        .expect("Failed to extract bid_status_auction from 'Some' value"),
                    index:   index.expect("Failed to extract index from 'Some' value"),
                })
            }
            BidStatus::Won => {
                if bid_status_auction.is_none() || index.is_none() {
                    return Err(anyhow::anyhow!(
                        "Won bid should have a tx_hash, auction_id and index"
                    ));
                }
                Ok(entities::BidStatusEvm::Won {
                    auction: bid_status_auction
                        .expect("Failed to extract bid_status_auction from 'Some' value"),
                    index:   index.expect("Failed to extract index from 'Some' value"),
                })
            }
            BidStatus::Lost => Ok(entities::BidStatusEvm::Lost {
                auction: bid_status_auction,
                index,
            }),
            BidStatus::Failed => Err(anyhow::anyhow!("Evm bid cannot be failed")),
            BidStatus::Expired => Err(anyhow::anyhow!("Evm bid cannot be expired")),
            BidStatus::Cancelled => Err(anyhow::anyhow!("Evm bid cannot be cancelled")),
        }
    }
    fn convert_bid_status(status: &entities::BidStatusEvm) -> BidStatus {
        match status {
            entities::BidStatusEvm::Pending => BidStatus::Pending,
            entities::BidStatusEvm::Submitted { .. } => BidStatus::Submitted,
            entities::BidStatusEvm::Lost { .. } => BidStatus::Lost,
            entities::BidStatusEvm::Won { .. } => BidStatus::Won,
        }
    }

    fn get_chain_data_entity(
        bid: &Bid<Evm>,
    ) -> anyhow::Result<<Evm as ChainTrait>::BidChainDataType> {
        Ok(entities::BidChainDataEvm {
            target_contract: bid.metadata.target_contract,
            target_calldata: bid.metadata.target_calldata.clone(),
            gas_limit:       U256::from(bid.metadata.gas_limit),
            permission_key:  Bytes::from(bid.permission_key.clone()),
        })
    }

    fn convert_permission_key(permission_key: &entities::PermissionKey<Evm>) -> Vec<u8> {
        permission_key.to_vec()
    }

    fn convert_amount(amount: &entities::BidAmountEvm) -> BigDecimal {
        BigDecimal::from_str(&amount.to_string()).expect("Failed to convert amount to BigDecimal")
    }

    fn get_metadata(chain_data: &entities::BidChainDataEvm) -> Self::BidMetadataType {
        BidMetadataEvm {
            target_contract: chain_data.target_contract,
            target_calldata: chain_data.target_calldata.clone(),
            bundle_index:    BundleIndex(None),
            gas_limit:       chain_data.gas_limit.as_u64(),
        }
    }

    fn get_update_bid_query(
        bid: &entities::Bid<Evm>,
        new_status: <Evm as ChainTrait>::BidStatusType,
    ) -> anyhow::Result<Query<'_, Postgres, PgArguments>> {
        match new_status {
            entities::BidStatusEvm::Pending => Err(anyhow::anyhow!("Cannot update bid status to pending")),
            entities::BidStatusEvm::Submitted { index, auction } => {
                Ok(sqlx::query!(
                    "UPDATE bid SET status = $1, auction_id = $2, metadata = jsonb_set(metadata, '{bundle_index}', $3) WHERE id = $4 AND status = $5",
                    BidStatus::Submitted as _,
                    auction.id,
                    json!(index),
                    bid.id,
                    BidStatus::Pending as _,
                ))
            }
            entities::BidStatusEvm::Lost { index, auction } => {
                match auction {
                    Some(auction) => {
                        match index {
                            Some(index) => {
                                Ok(sqlx::query!(
                                    "UPDATE bid SET status = $1, metadata = jsonb_set(metadata, '{bundle_index}', $2), auction_id = $3 WHERE id = $4 AND status = $5",
                                    BidStatus::Lost as _,
                                    json!(index),
                                    auction.id,
                                    bid.id,
                                    BidStatus::Submitted as _
                                ))
                            },
                            None => Ok(sqlx::query!(
                                "UPDATE bid SET status = $1, auction_id = $2 WHERE id = $3 AND status = $4",
                                BidStatus::Lost as _,
                                auction.id,
                                bid.id,
                                BidStatus::Pending as _,
                            )),
                        }
                    },
                    None => Ok(sqlx::query!(
                        "UPDATE bid SET status = $1 WHERE id = $2 AND status = $3",
                        BidStatus::Lost as _,
                        bid.id,
                        BidStatus::Pending as _
                    )),
                }
            },
            entities::BidStatusEvm::Won { index, .. } => Ok(sqlx::query!(
                "UPDATE bid SET status = $1, metadata = jsonb_set(metadata, '{bundle_index}', $2) WHERE id = $3 AND status = $4",
                BidStatus::Won as _,
                json!(index),
                bid.id,
                BidStatus::Submitted as _,
            )),
        }
    }
}

impl ModelTrait<Svm> for Svm {
    type BidMetadataType = BidMetadataSvm;

    fn get_chain_type() -> ChainType {
        ChainType::Svm
    }

    fn get_bid_status_auction_entity(
        auction: Option<Auction>,
    ) -> anyhow::Result<Option<entities::BidStatusAuction<entities::BidStatusSvm>>> {
        if let Some(auction) = auction {
            if let Some(tx_hash) = auction.tx_hash {
                let slice: [u8; 64] = tx_hash.try_into().map_err(|e| {
                    anyhow::anyhow!("Failed to convert svm transaction hash to slice {:?}", e)
                })?;
                return Ok(Some(entities::BidStatusAuction {
                    tx_hash: Signature::from(slice),
                    id:      auction.id,
                }));
            }
        }
        Ok(None)
    }

    fn get_bid_bundle_index(_bid: &Bid<Svm>) -> Option<u32> {
        None
    }

    fn get_bid_amount_entity(bid: &Bid<Svm>) -> anyhow::Result<entities::BidAmountSvm> {
        bid.bid_amount
            .to_string()
            .parse()
            .map_err(|e: ParseIntError| anyhow::anyhow!(e))
    }

    /// In SVM, the tx_hash is the signature of the transaction if the bid is submitted
    /// otherwise it is the signature of the transaction that caused the bid to be lost
    fn get_bid_status_entity(
        bid: &Bid<Svm>,
        auction: Option<Auction>,
    ) -> anyhow::Result<entities::BidStatusSvm> {
        let sig = *Self::get_chain_data_entity(bid)?
            .transaction
            .signatures
            .first()
            .ok_or_else(|| anyhow::anyhow!("Failed to get transaction signature"))?;
        match (bid.status.clone(), auction) {
            (BidStatus::Pending, _) => Ok(entities::BidStatusSvm::Pending),
            (BidStatus::Lost, auction) => Ok(entities::BidStatusSvm::Lost {
                auction: Self::get_bid_status_auction_entity(auction)?,
            }),
            (_, None) => Err(anyhow::anyhow!(
                "Bid with status {:?} should have an auction",
                bid.status
            )),

            (BidStatus::AwaitingSignature, Some(auction)) => {
                Ok(entities::BidStatusSvm::AwaitingSignature {
                    auction: entities::BidStatusAuction {
                        tx_hash: sig,
                        id:      auction.id,
                    },
                })
            }
            (BidStatus::Submitted, Some(auction)) => Ok(entities::BidStatusSvm::Submitted {
                auction: entities::BidStatusAuction {
                    tx_hash: sig,
                    id:      auction.id,
                },
            }),
            (BidStatus::Won, Some(auction)) => Ok(entities::BidStatusSvm::Won {
                auction: entities::BidStatusAuction {
                    tx_hash: sig,
                    id:      auction.id,
                },
            }),
            (BidStatus::Expired, Some(auction)) => Ok(entities::BidStatusSvm::Expired {
                auction: entities::BidStatusAuction {
                    tx_hash: sig,
                    id:      auction.id,
                },
            }),
            (BidStatus::Failed, Some(auction)) => Ok(entities::BidStatusSvm::Failed {
                auction: entities::BidStatusAuction {
                    tx_hash: sig,
                    id:      auction.id,
                },
            }),
            (BidStatus::Cancelled, Some(auction)) => Ok(entities::BidStatusSvm::Cancelled {
                auction: entities::BidStatusAuction {
                    tx_hash: sig,
                    id:      auction.id,
                },
            }),
        }
    }

    fn convert_bid_status(status: &entities::BidStatusSvm) -> BidStatus {
        match status {
            entities::BidStatusSvm::Pending => BidStatus::Pending,
            entities::BidStatusSvm::AwaitingSignature { .. } => BidStatus::AwaitingSignature,
            entities::BidStatusSvm::Submitted { .. } => BidStatus::Submitted,
            entities::BidStatusSvm::Lost { .. } => BidStatus::Lost,
            entities::BidStatusSvm::Won { .. } => BidStatus::Won,
            entities::BidStatusSvm::Failed { .. } => BidStatus::Failed,
            entities::BidStatusSvm::Expired { .. } => BidStatus::Expired,
            &entities::BidStatusSvm::Cancelled { .. } => BidStatus::Cancelled,
        }
    }

    fn get_chain_data_entity(bid: &Bid<Svm>) -> anyhow::Result<entities::BidChainDataSvm> {
        // The permission keys that are 64 bytes are the ones that are for submit_bid type.
        // These are stored in the database before adding the bid instruction type to the permission key svm.
        let slice: [u8; 65] =
            match bid.permission_key.len() {
                64 => {
                    let mut slice = [0; 65];
                    slice[0] = entities::BidPaymentInstructionType::SubmitBid.into();
                    slice[1..].copy_from_slice(&bid.permission_key);
                    Ok(slice)
                }
                _ => bid.permission_key.clone().try_into().map_err(|e| {
                    anyhow::anyhow!("Failed to convert permission key to slice {:?}", e)
                }),
            }?;

        let permission_key: PermissionKeySvm = PermissionKeySvm(slice);
        Ok(entities::BidChainDataSvm {
            transaction:                  bid.metadata.transaction.clone(),
            bid_payment_instruction_type:
                match entities::BidChainDataSvm::get_bid_payment_instruction_type(&permission_key) {
                    Some(bid_payment_instruction_type) => bid_payment_instruction_type,
                    None => {
                        return Err(anyhow::anyhow!(
                            "Failed to get bid payment instruction type from permission key, due to invalid data"
                        ))
                    }
                },
            router:                       entities::BidChainDataSvm::get_router(&permission_key),
            permission_account:           entities::BidChainDataSvm::get_permission_account(
                &permission_key,
            ),
        })
    }

    fn convert_permission_key(permission_key: &entities::PermissionKey<Svm>) -> Vec<u8> {
        permission_key.0.to_vec()
    }

    fn convert_amount(amount: &entities::BidAmountSvm) -> BigDecimal {
        (*amount).into()
    }

    fn get_metadata(chain_data: &<Svm as ChainTrait>::BidChainDataType) -> BidMetadataSvm {
        BidMetadataSvm {
            transaction: chain_data.transaction.clone(),
        }
    }

    fn get_update_bid_query(
        bid: &entities::Bid<Svm>,
        new_status: <Svm as ChainTrait>::BidStatusType,
    ) -> anyhow::Result<Query<'_, Postgres, PgArguments>> {
        let now = OffsetDateTime::now_utc();
        match &new_status {
            entities::BidStatusSvm::Pending => {
                Err(anyhow::anyhow!("Cannot update bid status to pending"))
            }
            entities::BidStatusSvm::AwaitingSignature { auction } => Ok(sqlx::query!(
                "UPDATE bid SET status = $1, auction_id = $2 WHERE id = $3 AND status = $4",
                BidStatus::AwaitingSignature as _,
                auction.id,
                bid.id,
                BidStatus::Pending as _,
            )),
            entities::BidStatusSvm::Submitted { auction } => Ok(sqlx::query!(
                "UPDATE bid SET status = $1, auction_id = $2 WHERE id = $3 AND status IN ($4, $5)",
                BidStatus::Submitted as _,
                auction.id,
                bid.id,
                BidStatus::Pending as _,
                BidStatus::AwaitingSignature as _,
            )),
            entities::BidStatusSvm::Lost { auction: Some(auction) } => Ok(sqlx::query!(
                    "UPDATE bid SET status = $1, auction_id = $2, conclusion_time = $3 WHERE id = $4 AND status = $5",
                    BidStatus::Lost as _,
                    auction.id,
                    PrimitiveDateTime::new(now.date(), now.time()),
                    bid.id,
                    BidStatus::Pending as _
                )),
            entities::BidStatusSvm::Lost { auction: None } => Ok(sqlx::query!(
                    "UPDATE bid SET status = $1, conclusion_time = $2 WHERE id = $3 AND status = $4",
                    BidStatus::Lost as _,
                    PrimitiveDateTime::new(now.date(), now.time()),
                    bid.id,
                    BidStatus::Pending as _
                )),
            entities::BidStatusSvm::Won { .. } | entities::BidStatusSvm::Expired { .. } | entities::BidStatusSvm::Failed { .. }  => Ok(sqlx::query!(
                "UPDATE bid SET status = $1, conclusion_time = $2 WHERE id = $3 AND status IN ($4, $5)",
                Self::convert_bid_status(&new_status) as _,
                PrimitiveDateTime::new(now.date(), now.time()),
                bid.id,
                BidStatus::Submitted as _,
                BidStatus::AwaitingSignature as _,
            )),
            entities::BidStatusSvm::Cancelled { auction } => Ok(sqlx::query!(
                "UPDATE bid SET status = $1, conclusion_time = $2, auction_id = $3 WHERE id = $4 AND status = $5",
                BidStatus::Cancelled as _,
                PrimitiveDateTime::new(now.date(), now.time()),
                auction.id,
                bid.id,
                BidStatus::AwaitingSignature as _,
            )),
        }
    }
}
#[derive(Clone, Debug, FromRow)]
pub struct Bid<T: ChainTrait + ModelTrait<T>> {
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
    #[allow(dead_code)]
    pub conclusion_time: Option<PrimitiveDateTime>,
    pub profile_id:      Option<ProfileId>,
    pub metadata:        Json<T::BidMetadataType>,
}

impl<T: ChainTrait + ModelTrait<T>> Bid<T> {
    pub fn new(
        bid: entities::BidCreate<T>,
        amount: &T::BidAmountType,
        chain_data: &T::BidChainDataType,
    ) -> Bid<T> {
        let now = OffsetDateTime::now_utc();
        Bid {
            id:              entities::BidId::new_v4(),
            creation_time:   PrimitiveDateTime::new(now.date(), now.time()),
            permission_key:  T::convert_permission_key(&chain_data.get_permission_key()),
            chain_id:        bid.chain_id.clone(),
            chain_type:      T::get_chain_type(),
            bid_amount:      T::convert_amount(amount),
            status:          BidStatus::Pending,
            auction_id:      None,
            initiation_time: PrimitiveDateTime::new(
                bid.initiation_time.date(),
                bid.initiation_time.time(),
            ),
            conclusion_time: None,
            profile_id:      bid.profile.map(|p| p.id),
            metadata:        Json(T::get_metadata(chain_data)),
        }
    }

    pub fn get_bid_entity(&self, auction: Option<Auction>) -> anyhow::Result<entities::Bid<T>> {
        Ok(entities::Bid {
            id:              self.id,
            chain_id:        self.chain_id.clone(),
            initiation_time: self.initiation_time.assume_offset(UtcOffset::UTC),
            profile_id:      self.profile_id,

            amount:     T::get_bid_amount_entity(self)?,
            status:     T::get_bid_status_entity(self, auction)?,
            chain_data: T::get_chain_data_entity(self)?,
        })
    }
}
