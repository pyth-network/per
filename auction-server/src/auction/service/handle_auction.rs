use {
    super::{
        auction_manager::AuctionManager,
        Service,
    },
    crate::{
        auction::{
            entities::{
                self,
                BidStatus,
                BidStatusSvm,
            },
            service::{
                add_auction::AddAuctionInput,
                update_bid_status::UpdateBidStatusInput,
            },
        },
        kernel::entities::PermissionKeySvm,
    },
    futures::future::join_all,
    time::OffsetDateTime,
    tokio::sync::MutexGuard,
};

pub struct HandleAuctionInput {
    pub permission_key: PermissionKeySvm,
}

impl Service {
    #[tracing::instrument(skip_all, fields(auction_id, bid_ids, winner_bid_ids), err(level = tracing::Level::TRACE))]
    async fn submit_auction<'a>(
        &self,
        auction: entities::Auction,
        _auction_mutex_guard: MutexGuard<'a, ()>,
    ) -> anyhow::Result<()> {
        tracing::Span::current().record("auction_id", auction.id.to_string());
        tracing::Span::current().record(
            "bid_ids",
            tracing::field::display(entities::BidContainerTracing(&auction.bids)),
        );

        let permission_key = auction.permission_key.clone();
        if !auction.is_ready(Service::AUCTION_MINIMUM_LIFETIME) {
            tracing::info!(
                permission_key = permission_key.to_string(),
                "Auction is not ready yet"
            );
            return Ok(());
        }

        let winner_bids = self.get_winner_bids(&auction).await?;
        tracing::Span::current().record(
            "winner_bid_ids",
            tracing::field::display(entities::BidContainerTracing(&winner_bids)),
        );
        if winner_bids.is_empty() {
            join_all(auction.bids.into_iter().map(|bid| {
                self.update_bid_status(UpdateBidStatusInput {
                    bid,
                    new_status: BidStatusSvm::new_lost(),
                })
            }))
            .await;
            return Ok(());
        }

        let auction = self
            .add_auction(AddAuctionInput { auction })
            .await
            .map_err(|err| {
                tracing::error!(error = ?err, "Failed to add auction");
                anyhow::anyhow!("Failed to add auction")
            })?;

        tracing::info!(
            auction = ?auction,
            chain_id = self.config.chain_id,
            "Auction submission started",
        );

        match self
            .submit_bids(permission_key.clone(), winner_bids.clone())
            .await
        {
            Ok(tx_hash) => {
                tracing::debug!(tx_hash = ?tx_hash, "Submitted transaction");
                let auction = self.repo.submit_auction(auction, tx_hash).await?;
                join_all(auction.bids.iter().map(|bid| {
                    self.update_bid_status(UpdateBidStatusInput {
                        new_status: Service::get_new_status(
                            bid,
                            &winner_bids,
                            entities::BidStatusAuction {
                                tx_hash,
                                id: auction.id,
                            },
                        ),
                        bid:        bid.clone(),
                    })
                }))
                .await;
            }
            Err(err) => {
                tracing::error!(error = ?err, "Transaction failed to submit");
            }
        };
        Ok(())
    }

    #[tracing::instrument(skip_all, fields(bid_ids, auction_id))]
    async fn submit_auction_for_lock(
        &self,
        permission_key: &PermissionKeySvm,
        auction_lock: entities::AuctionLock,
    ) -> anyhow::Result<()> {
        let acquired_lock = auction_lock.lock().await;

        let bid_collection_time = OffsetDateTime::now_utc();
        let bids = self
            .repo
            .get_in_memory_pending_bids_by_permission_key(permission_key)
            .await;

        tracing::Span::current().record(
            "bid_ids",
            tracing::field::display(entities::BidContainerTracing(&bids)),
        );

        match entities::Auction::try_new(bids, bid_collection_time) {
            Some(auction) => {
                tracing::Span::current().record("auction_id", auction.id.to_string());
                self.submit_auction(auction, acquired_lock).await
            }
            None => Ok(()),
        }
    }

    pub async fn handle_auction(&self, input: HandleAuctionInput) -> anyhow::Result<()> {
        tracing::info!(
            chain_id = self.config.chain_id,
            permission_key = input.permission_key.to_string(),
            "Handling auction",
        );
        let permission_key = input.permission_key;
        match self.get_submission_state(&permission_key).await {
            entities::SubmitType::ByOther => Ok(()),
            entities::SubmitType::ByServer => {
                let auction_lock = self
                    .repo
                    .get_or_create_in_memory_auction_lock(permission_key.clone())
                    .await;
                let result = self
                    .submit_auction_for_lock(&permission_key, auction_lock)
                    .await;
                self.repo
                    .remove_in_memory_auction_lock(&permission_key)
                    .await;
                result
            }
            entities::SubmitType::Invalid => {
                // Fetch all pending bids and mark them as lost
                let bids: Vec<entities::Bid> = self
                    .repo
                    .get_in_memory_pending_bids_by_permission_key(&permission_key)
                    .await
                    .into_iter()
                    .filter(|bid| bid.status.is_pending())
                    .collect();
                join_all(bids.into_iter().map(|bid| {
                    self.update_bid_status(UpdateBidStatusInput {
                        bid,
                        new_status: BidStatusSvm::new_lost(),
                    })
                }))
                .await;
                Ok(())
            }
        }
    }
}
