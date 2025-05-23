#[cfg(test)]
use mockall::automock;
use {
    super::{
        entities::{
            self,
            BidStatus as _,
        },
        AnalyticsDatabaseInserter,
    },
    crate::{
        api::RestError,
        auction::entities::BidStatusSvm,
        kernel::{
            db::DB,
            entities::{
                ChainId,
                PermissionKeySvm,
                Svm,
            },
        },
        models::ProfileId,
    },
    axum::async_trait,
    serde::{
        Deserialize,
        Serialize,
    },
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
        QueryBuilder,
    },
    std::{
        fmt::Debug,
        num::ParseIntError,
        ops::Deref,
    },
    time::{
        OffsetDateTime,
        PrimitiveDateTime,
        UtcOffset,
    },
    tracing::instrument,
    uuid::Uuid,
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

#[derive(Clone, Debug, PartialEq, PartialOrd, sqlx::Type, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "bid_status", rename_all = "snake_case")]
pub enum BidStatus {
    Pending,
    AwaitingSignature,
    SentToUserForSubmission,
    Submitted,
    Lost,
    Won,
    Failed,
    Expired,
    Cancelled,
    SubmissionFailedCancelled,
    SubmissionFailedDeadlinePassed,
}

#[derive(Clone, Debug, PartialEq, PartialOrd, sqlx::Type, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "status_reason", rename_all = "snake_case")]
pub enum BidStatusReason {
    InsufficientUserFunds,
    InsufficientSearcherFunds,
    InsufficientFundsSolTransfer,
    DeadlinePassed,
    Other,
}

impl From<BidStatusReason> for entities::BidFailedReason {
    fn from(reason: BidStatusReason) -> Self {
        match reason {
            BidStatusReason::DeadlinePassed => entities::BidFailedReason::DeadlinePassed,
            BidStatusReason::InsufficientUserFunds => {
                entities::BidFailedReason::InsufficientUserFunds
            }
            BidStatusReason::InsufficientSearcherFunds => {
                entities::BidFailedReason::InsufficientSearcherFunds
            }
            BidStatusReason::InsufficientFundsSolTransfer => {
                entities::BidFailedReason::InsufficientFundsSolTransfer
            }
            BidStatusReason::Other => entities::BidFailedReason::Other,
        }
    }
}

impl From<entities::BidFailedReason> for BidStatusReason {
    fn from(reason: entities::BidFailedReason) -> Self {
        match reason {
            entities::BidFailedReason::DeadlinePassed => BidStatusReason::DeadlinePassed,
            entities::BidFailedReason::InsufficientUserFunds => {
                BidStatusReason::InsufficientUserFunds
            }
            entities::BidFailedReason::InsufficientSearcherFunds => {
                BidStatusReason::InsufficientSearcherFunds
            }
            entities::BidFailedReason::InsufficientFundsSolTransfer => {
                BidStatusReason::InsufficientFundsSolTransfer
            }
            entities::BidFailedReason::Other => BidStatusReason::Other,
        }
    }
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
pub struct BidMetadataSvm {
    #[serde(with = "express_relay_api_types::serde::transaction_svm")]
    pub transaction: VersionedTransaction,
}

impl Svm {
    fn get_chain_type() -> ChainType {
        ChainType::Svm
    }

    fn get_bid_status_auction_entity(
        auction: Option<Auction>,
    ) -> anyhow::Result<Option<entities::BidStatusAuction>> {
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

    fn get_bid_amount_entity(bid: &Bid) -> anyhow::Result<entities::BidAmountSvm> {
        bid.bid_amount
            .to_string()
            .parse()
            .map_err(|e: ParseIntError| anyhow::anyhow!(e))
    }

    pub fn get_bid_status_reason(status: &entities::BidStatusSvm) -> Option<BidStatusReason> {
        match status {
            entities::BidStatusSvm::Pending => None,
            entities::BidStatusSvm::AwaitingSignature { .. } => None,
            entities::BidStatusSvm::SentToUserForSubmission { .. } => None,
            entities::BidStatusSvm::Submitted { .. } => None,
            entities::BidStatusSvm::Lost { .. } => None,
            entities::BidStatusSvm::Won { .. } => None,
            entities::BidStatusSvm::Failed { reason, .. } => reason.clone().map(|r| r.into()),
            entities::BidStatusSvm::Expired { .. } => None,
            entities::BidStatusSvm::Cancelled { .. } => None,
            entities::BidStatusSvm::SubmissionFailed { .. } => None,
        }
    }

    /// In SVM, the tx_hash is the signature of the transaction if the bid is submitted
    /// otherwise it is the signature of the transaction that caused the bid to be lost
    fn get_bid_status_entity(
        bid: &Bid,
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
            (BidStatus::SentToUserForSubmission, Some(auction)) => {
                Ok(entities::BidStatusSvm::SentToUserForSubmission {
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
                reason:  bid.status_reason.clone().map(|r| r.into()),
            }),
            (BidStatus::Cancelled, Some(auction)) => Ok(entities::BidStatusSvm::Cancelled {
                auction: entities::BidStatusAuction {
                    tx_hash: sig,
                    id:      auction.id,
                },
            }),
            (BidStatus::SubmissionFailedCancelled, Some(auction)) => {
                Ok(entities::BidStatusSvm::SubmissionFailed {
                    auction: entities::BidStatusAuction {
                        tx_hash: sig,
                        id:      auction.id,
                    },
                    reason:  entities::BidSubmissionFailedReason::Cancelled,
                })
            }
            (BidStatus::SubmissionFailedDeadlinePassed, Some(auction)) => {
                Ok(entities::BidStatusSvm::SubmissionFailed {
                    auction: entities::BidStatusAuction {
                        tx_hash: sig,
                        id:      auction.id,
                    },
                    reason:  entities::BidSubmissionFailedReason::DeadlinePassed,
                })
            }
        }
    }

    pub fn convert_bid_status(status: &entities::BidStatusSvm) -> BidStatus {
        match status {
            entities::BidStatusSvm::Pending => BidStatus::Pending,
            entities::BidStatusSvm::AwaitingSignature { .. } => BidStatus::AwaitingSignature,
            entities::BidStatusSvm::SentToUserForSubmission { .. } => {
                BidStatus::SentToUserForSubmission
            }
            entities::BidStatusSvm::Submitted { .. } => BidStatus::Submitted,
            entities::BidStatusSvm::Lost { .. } => BidStatus::Lost,
            entities::BidStatusSvm::Won { .. } => BidStatus::Won,
            entities::BidStatusSvm::Failed { .. } => BidStatus::Failed,
            entities::BidStatusSvm::Expired { .. } => BidStatus::Expired,
            entities::BidStatusSvm::Cancelled { .. } => BidStatus::Cancelled,
            entities::BidStatusSvm::SubmissionFailed { reason, .. } => match reason {
                entities::BidSubmissionFailedReason::Cancelled => {
                    BidStatus::SubmissionFailedCancelled
                }
                entities::BidSubmissionFailedReason::DeadlinePassed => {
                    BidStatus::SubmissionFailedDeadlinePassed
                }
            },
        }
    }

    fn get_chain_data_entity(bid: &Bid) -> anyhow::Result<entities::BidChainDataSvm> {
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

    fn convert_permission_key(permission_key: &PermissionKeySvm) -> Vec<u8> {
        permission_key.0.to_vec()
    }

    fn convert_amount(amount: &entities::BidAmountSvm) -> BigDecimal {
        (*amount).into()
    }

    fn get_metadata(chain_data: &entities::BidChainDataSvm) -> BidMetadataSvm {
        BidMetadataSvm {
            transaction: chain_data.transaction.clone(),
        }
    }

    fn get_update_bid_query(
        bid: &entities::Bid,
        new_status: entities::BidStatusSvm,
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
            entities::BidStatusSvm::SentToUserForSubmission { auction } => Ok(sqlx::query!(
                "UPDATE bid SET status = $1, auction_id = $2 WHERE id = $3 AND status = $4",
                BidStatus::SentToUserForSubmission as _,
                auction.id,
                bid.id,
                BidStatus::Pending as _,
            )),
            entities::BidStatusSvm::Submitted { auction } => Ok(sqlx::query!(
                "UPDATE bid SET status = $1, auction_id = $2 WHERE id = $3 AND status IN ($4, $5, $6)",
                BidStatus::Submitted as _,
                auction.id,
                bid.id,
                BidStatus::Pending as _,
                BidStatus::AwaitingSignature as _,
                BidStatus::SentToUserForSubmission as _,
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
            entities::BidStatusSvm::Won { .. }  |  entities::BidStatusSvm::Failed { reason: None, .. } => Ok(sqlx::query!(
                "UPDATE bid SET status = $1, conclusion_time = $2 WHERE id = $3 AND status IN ($4, $5)",
                Self::convert_bid_status(&new_status) as _,
                PrimitiveDateTime::new(now.date(), now.time()),
                bid.id,
                BidStatus::Submitted as _,
                BidStatus::SentToUserForSubmission as _,
            )),
            entities::BidStatusSvm::Failed { reason : Some(reason), .. } => Ok(sqlx::query!(
                "UPDATE bid SET status = $1, conclusion_time = $2, status_reason = $3 WHERE id = $4 AND status IN ($5, $6)",
                Self::convert_bid_status(&new_status) as _,
                PrimitiveDateTime::new(now.date(), now.time()),
                BidStatusReason::from(reason.clone()) as _,
                bid.id,
                BidStatus::Submitted as _,
                BidStatus::SentToUserForSubmission as _,
            )),
            entities::BidStatusSvm::Expired { auction } => Ok(sqlx::query!(
                "UPDATE bid SET status = $1, conclusion_time = $2, auction_id = $3 WHERE id = $4 AND status IN ($5, $6, $7, $8)",
                BidStatus::Expired as _,
                PrimitiveDateTime::new(now.date(), now.time()),
                auction.id,
                bid.id,
                BidStatus::Pending as _,
                BidStatus::Submitted as _,
                BidStatus::AwaitingSignature as _,
                BidStatus::SentToUserForSubmission as _,
            )),
            entities::BidStatusSvm::Cancelled { auction } => Ok(sqlx::query!(
                "UPDATE bid SET status = $1, conclusion_time = $2, auction_id = $3 WHERE id = $4 AND status = $5",
                BidStatus::Cancelled as _,
                PrimitiveDateTime::new(now.date(), now.time()),
                auction.id,
                bid.id,
                BidStatus::AwaitingSignature as _,
            )),
            entities::BidStatusSvm::SubmissionFailed { auction, reason } => {
                Ok(match reason {
                    entities::BidSubmissionFailedReason::Cancelled => {
                        sqlx::query!(
                            "UPDATE bid SET status = $1, conclusion_time = $2, auction_id = $3 WHERE id = $4 AND status = $5",
                            BidStatus::SubmissionFailedCancelled as _,
                            PrimitiveDateTime::new(now.date(), now.time()),
                            auction.id,
                            bid.id,
                            BidStatus::Cancelled as _,
                        )
                    },
                    &entities::BidSubmissionFailedReason::DeadlinePassed => {
                        sqlx::query!(
                            "UPDATE bid SET status = $1, conclusion_time = $2, auction_id = $3 WHERE id = $4 AND status IN ($5, $6, $7)",
                            BidStatus::SubmissionFailedDeadlinePassed as _,
                            PrimitiveDateTime::new(now.date(), now.time()),
                            auction.id,
                            bid.id,
                            BidStatus::AwaitingSignature as _,
                            BidStatus::SentToUserForSubmission as _,
                            BidStatus::Cancelled as _,
                        )
                    }
                })

            },
        }
    }
}
#[derive(Clone, Debug, FromRow)]
pub struct Bid {
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
    pub metadata:        Json<BidMetadataSvm>,
    pub status_reason:   Option<BidStatusReason>,
}

impl Bid {
    pub fn new(
        bid: entities::BidCreate,
        amount: &entities::BidAmountSvm,
        chain_data: &entities::BidChainDataSvm,
    ) -> Bid {
        let now = OffsetDateTime::now_utc();
        Bid {
            id:              entities::BidId::new_v4(),
            creation_time:   PrimitiveDateTime::new(now.date(), now.time()),
            permission_key:  Svm::convert_permission_key(&chain_data.get_permission_key()),
            chain_id:        bid.chain_id.clone(),
            chain_type:      Svm::get_chain_type(),
            bid_amount:      Svm::convert_amount(amount),
            status:          BidStatus::Pending,
            auction_id:      None,
            initiation_time: PrimitiveDateTime::new(
                bid.initiation_time.date(),
                bid.initiation_time.time(),
            ),
            conclusion_time: None,
            profile_id:      bid.profile.map(|p| p.id),
            metadata:        Json(Svm::get_metadata(chain_data)),
            status_reason:   None,
        }
    }

    pub fn get_bid_entity(&self, auction: Option<Auction>) -> anyhow::Result<entities::Bid> {
        Ok(entities::Bid {
            id:              self.id,
            chain_id:        self.chain_id.clone(),
            initiation_time: self.initiation_time.assume_offset(UtcOffset::UTC),
            creation_time:   self.creation_time.assume_offset(UtcOffset::UTC),
            conclusion_time: self
                .conclusion_time
                .map(|t| t.assume_offset(UtcOffset::UTC)),
            profile_id:      self.profile_id,

            amount:     Svm::get_bid_amount_entity(self)?,
            status:     Svm::get_bid_status_entity(self, auction)?,
            chain_data: Svm::get_chain_data_entity(self)?,
        })
    }
}


#[cfg_attr(test, automock)]
#[async_trait]
pub trait Database: Debug + Send + Sync + 'static {
    async fn add_auction(&self, auction: &entities::Auction) -> anyhow::Result<()>;
    async fn add_bid(&self, bid: &Bid) -> Result<(), RestError>;
    async fn conclude_auction(&self, auction_id: entities::AuctionId) -> anyhow::Result<()>;
    async fn get_bid(&self, bid_id: entities::BidId, chain_id: ChainId) -> Result<Bid, RestError>;
    async fn get_auction(&self, auction_id: entities::AuctionId) -> Result<Auction, RestError>;
    async fn get_auctions_by_bids(&self, bids: &[Bid]) -> Result<Vec<Auction>, RestError>;
    async fn get_bids(
        &self,
        chain_id: ChainId,
        profile_id: ProfileId,
        from_time: Option<OffsetDateTime>,
    ) -> Result<Vec<Bid>, RestError>;
    async fn submit_auction(
        &self,
        auction: &entities::Auction,
        transaction_hash: &Signature,
    ) -> anyhow::Result<Option<entities::Auction>>;
    async fn update_bid_status(
        &self,
        bid: &entities::Bid,
        new_status: &BidStatusSvm,
    ) -> anyhow::Result<bool>;
}

#[async_trait]
impl Database for DB {
    #[instrument(
        target = "metrics",
        name = "db_add_auction",
        fields(
            category = "db_queries",
            result = "success",
            name = "add_auction",
            tracing_enabled
        ),
        skip_all
    )]
    async fn add_auction(&self, auction: &entities::Auction) -> anyhow::Result<()> {
        sqlx::query!(
            "INSERT INTO auction (id, creation_time, permission_key, chain_id, chain_type, bid_collection_time, tx_hash) VALUES ($1, $2, $3, $4, $5, $6, $7)",
            auction.id,
            PrimitiveDateTime::new(auction.creation_time.date(), auction.creation_time.time()),
            Svm::convert_permission_key(&auction.permission_key),
            auction.chain_id,
            Svm::get_chain_type() as _,
            PrimitiveDateTime::new(auction.bid_collection_time.date(), auction.bid_collection_time.time()),
            auction.tx_hash.clone().map(|tx_hash| BidStatusSvm::convert_tx_hash(&tx_hash)),
        )
        .execute(self)
        .await
        .inspect_err(|_| {
            tracing::Span::current().record("result", "error");
        })?;
        Ok(())
    }

    #[instrument(
        target = "metrics",
        name = "db_add_bid",
        fields(
            category = "db_queries",
            result = "success",
            name = "add_bid",
            tracing_enabled
        ),
        skip_all
    )]
    async fn add_bid(&self, bid: &Bid) -> Result<(), RestError> {
        sqlx::query!("INSERT INTO bid (id, creation_time, permission_key, chain_id, chain_type, bid_amount, status, initiation_time, profile_id, metadata) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
            bid.id,
            bid.creation_time,
            bid.permission_key,
            bid.chain_id,
            bid.chain_type as _,
            bid.bid_amount,
            bid.status as _,
            bid.initiation_time,
            bid.profile_id,
            serde_json::to_value(bid.metadata.clone()).expect("Failed to serialize metadata"),
        ).execute(self)
        .await
        .map_err(|e| {
            tracing::Span::current().record("result", "error");
            tracing::error!(error = e.to_string(), bid = ?bid, "DB: Failed to insert bid");
            RestError::TemporarilyUnavailable
        })?;
        Ok(())
    }

    #[instrument(
        target = "metrics",
        name = "db_conclude_auction",
        fields(
            category = "db_queries",
            result = "success",
            name = "conclude_auction",
            tracing_enabled
        ),
        skip_all
    )]
    async fn conclude_auction(&self, auction_id: entities::AuctionId) -> anyhow::Result<()> {
        let now = OffsetDateTime::now_utc();
        sqlx::query!(
            "UPDATE auction SET conclusion_time = $1 WHERE id = $2 AND conclusion_time IS NULL",
            PrimitiveDateTime::new(now.date(), now.time()),
            auction_id,
        )
        .execute(self)
        .await
        .inspect_err(|_| {
            tracing::Span::current().record("result", "error");
        })?;
        Ok(())
    }

    #[instrument(
        target = "metrics",
        name = "db_get_bid",
        fields(
            category = "db_queries",
            result = "success",
            name = "get_bid",
            tracing_enabled
        ),
        skip_all
    )]
    async fn get_bid(&self, bid_id: entities::BidId, chain_id: ChainId) -> Result<Bid, RestError> {
        sqlx::query_as("SELECT * FROM bid WHERE id = $1 AND chain_id = $2")
            .bind(bid_id)
            .bind(&chain_id)
            .fetch_one(self)
            .await
            .map_err(|e| match e {
                sqlx::Error::RowNotFound => RestError::BidNotFound,
                _ => {
                    tracing::Span::current().record("result", "error");
                    tracing::error!(
                        error = e.to_string(),
                        bid_id = bid_id.to_string(),
                        "Failed to get bid from db"
                    );
                    RestError::TemporarilyUnavailable
                }
            })
    }

    #[instrument(
        target = "metrics",
        name = "db_get_auction",
        fields(
            category = "db_queries",
            result = "success",
            name = "get_auction",
            tracing_enabled
        ),
        skip_all
    )]
    async fn get_auction(&self, auction_id: entities::AuctionId) -> Result<Auction, RestError> {
        sqlx::query_as("SELECT * FROM auction WHERE id = $1")
            .bind(auction_id)
            .fetch_one(self)
            .await
            .map_err(|e| {
                tracing::Span::current().record("result", "error");
                tracing::error!(
                    error = e.to_string(),
                    auction_id = auction_id.to_string(),
                    "Failed to get auction from db"
                );
                RestError::TemporarilyUnavailable
            })
    }

    #[instrument(
        target = "metrics",
        name = "db_get_auctions_by_bids",
        fields(
            category = "db_queries",
            result = "success",
            name = "get_auctions_by_bids",
            tracing_enabled
        ),
        skip_all
    )]
    async fn get_auctions_by_bids(&self, bids: &[Bid]) -> Result<Vec<Auction>, RestError> {
        let auction_ids: Vec<entities::AuctionId> =
            bids.iter().filter_map(|bid| bid.auction_id).collect();
        sqlx::query_as("SELECT * FROM auction WHERE id = ANY($1)")
            .bind(auction_ids)
            .fetch_all(self)
            .await
            .map_err(|e| {
                tracing::Span::current().record("result", "error");
                tracing::error!("DB: Failed to fetch auctions: {}", e);
                RestError::TemporarilyUnavailable
            })
    }

    #[instrument(
        target = "metrics",
        name = "db_get_bids",
        fields(
            category = "db_queries",
            result = "success",
            name = "get_bids",
            tracing_enabled
        ),
        skip_all
    )]
    async fn get_bids(
        &self,
        chain_id: ChainId,
        profile_id: ProfileId,
        from_time: Option<OffsetDateTime>,
    ) -> Result<Vec<Bid>, RestError> {
        let mut query = QueryBuilder::new("SELECT * from bid where profile_id = ");
        query
            .push_bind(profile_id)
            .push(" AND chain_id = ")
            .push_bind(chain_id);
        if let Some(from_time) = from_time {
            query.push(" AND initiation_time >= ");
            query.push_bind(from_time);
        }
        query.push(" ORDER BY initiation_time ASC LIMIT 20");
        query.build_query_as().fetch_all(self).await.map_err(|e| {
            tracing::Span::current().record("result", "error");
            tracing::error!("DB: Failed to fetch bids: {}", e);
            RestError::TemporarilyUnavailable
        })
    }

    #[instrument(
        target = "metrics",
        name = "db_submit_auction",
        fields(
            category = "db_queries",
            result = "success",
            name = "submit_auction",
            tracing_enabled
        ),
        skip_all
    )]
    async fn submit_auction(
        &self,
        auction: &entities::Auction,
        transaction_hash: &Signature,
    ) -> anyhow::Result<Option<entities::Auction>> {
        let mut auction = auction.clone();
        let now = OffsetDateTime::now_utc();
        auction.tx_hash = Some(*transaction_hash);
        auction.submission_time = Some(now);
        let result = sqlx::query!("UPDATE auction SET submission_time = $1, tx_hash = $2 WHERE id = $3 AND submission_time IS NULL",
            PrimitiveDateTime::new(now.date(), now.time()),
            BidStatusSvm::convert_tx_hash(transaction_hash),
            auction.id,
        ).execute(self).await.inspect_err(|_| {
            tracing::Span::current().record("result", "error");
        })?;
        Ok((result.rows_affected() != 0).then_some(auction))
    }

    #[instrument(
        target = "metrics",
        name = "db_update_bid_status",
        fields(
            category = "db_queries",
            result = "success",
            name = "update_bid_status",
            tracing_enabled
        ),
        skip_all
    )]
    async fn update_bid_status(
        &self,
        bid: &entities::Bid,
        new_status: &entities::BidStatusSvm,
    ) -> anyhow::Result<bool> {
        let update_query = Svm::get_update_bid_query(bid, new_status.clone())?;
        let result = update_query.execute(self).await.inspect_err(|_| {
            tracing::Span::current().record("result", "error");
        })?;
        Ok(result.rows_affected() > 0)
    }
}

#[derive(clickhouse::Row, Serialize, Deserialize, Debug)]
pub struct BidAnalyticsSwap {
    #[serde(with = "clickhouse::serde::uuid")]
    pub id:              Uuid,
    #[serde(with = "clickhouse::serde::time::datetime64::micros")]
    pub creation_time:   OffsetDateTime,
    #[serde(with = "clickhouse::serde::time::datetime64::micros")]
    pub initiation_time: OffsetDateTime,
    pub permission_key:  String,
    pub chain_id:        String,
    pub transaction:     String,
    pub bid_amount:      u64,

    #[serde(with = "clickhouse::serde::uuid::option")]
    pub auction_id:      Option<Uuid>,
    #[serde(with = "clickhouse::serde::uuid::option")]
    pub opportunity_id:  Option<Uuid>,
    #[serde(with = "clickhouse::serde::time::datetime64::micros::option")]
    pub conclusion_time: Option<OffsetDateTime>,

    pub searcher_token_mint:      String,
    pub searcher_token_amount:    u64,
    pub searcher_token_usd_price: Option<f64>,

    pub user_token_mint:      String,
    pub user_token_amount:    u64,
    pub user_token_usd_price: Option<f64>,

    pub status:        String,
    pub status_reason: Option<String>,

    pub user_wallet_address:     String,
    pub searcher_wallet_address: String,
    pub fee_token:               String,
    pub referral_fee_ppm:        u64,
    pub platform_fee_ppm:        u64,
    pub deadline:                i64,
    pub token_program_user:      String,
    pub token_program_searcher:  String,
    pub router_token_account:    String,

    #[serde(with = "clickhouse::serde::uuid::option")]
    pub profile_id: Option<Uuid>,
}

#[derive(clickhouse::Row, Serialize, Deserialize, Debug)]
pub struct BidAnalyticsLimo {
    #[serde(with = "clickhouse::serde::uuid")]
    pub id:              Uuid,
    #[serde(with = "clickhouse::serde::time::datetime64::micros")]
    pub creation_time:   OffsetDateTime,
    #[serde(with = "clickhouse::serde::time::datetime64::micros")]
    pub initiation_time: OffsetDateTime,
    pub permission_key:  String,
    pub chain_id:        String,
    pub transaction:     String,
    pub bid_amount:      u64,

    #[serde(with = "clickhouse::serde::uuid::option")]
    pub auction_id:      Option<Uuid>,
    #[serde(with = "clickhouse::serde::time::datetime64::micros::option")]
    pub conclusion_time: Option<OffsetDateTime>,

    pub status: String,

    pub router:             String,
    pub permission_account: String,
    pub deadline:           i64,

    #[serde(with = "clickhouse::serde::uuid::option")]
    pub profile_id: Option<Uuid>,
}

#[allow(clippy::large_enum_variant)]
pub enum BidAnalytics {
    Swap(BidAnalyticsSwap),
    Limo(BidAnalyticsLimo),
}

#[cfg_attr(test, automock)]
#[async_trait]
pub trait AnalyticsDatabase: Debug + Send + Sync + 'static {
    async fn add_bid(&self, bid: BidAnalytics) -> Result<(), anyhow::Error>;
}

#[async_trait]
impl AnalyticsDatabase for AnalyticsDatabaseInserter {
    #[instrument(
        target = "metrics",
        name = "db_analytics_add_bid",
        fields(
            category = "db_analytics_queries",
            result = "success",
            name = "add_bid",
            tracing_enabled
        ),
        skip_all
    )]
    async fn add_bid(&self, bid: BidAnalytics) -> Result<(), anyhow::Error> {
        match bid {
            BidAnalytics::Swap(bid) => self
                .inserter_bid_swap
                .sender
                .send(bid)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to send swap bid analytics {:?}", e)),
            BidAnalytics::Limo(bid) => self
                .inserter_bid_limo
                .sender
                .send(bid)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to send limo bid analytics {:?}", e)),
        }
    }
}
