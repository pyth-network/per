use {
    super::{
        auction_manager::AuctionManager,
        ChainTrait,
        Service,
    },
    crate::auction::{
        entities::{
            self,
            BidStatus,
        },
        service::update_bid_status::UpdateBidStatusInput,
    },
    futures::future::join_all,
    time::OffsetDateTime,
    tokio::sync::MutexGuard,
};

pub struct HandleAuctionInput<T: ChainTrait> {
    pub permission_key: entities::PermissionKey<T>,
}

impl<T: ChainTrait> Service<T>
where
    Service<T>: AuctionManager<T>,
{
    async fn submit_auction<'a>(
        &self,
        auction: entities::Auction<T>,
        _auction_mutex_gaurd: MutexGuard<'a, ()>,
    ) -> anyhow::Result<()> {
        let permission_key = auction.permission_key.clone();
        if !auction.is_ready(Service::AUCTION_MINIMUM_LIFETIME) {
            tracing::info!(permission_key = ?permission_key, "Auction is not ready yet");
            return Ok(());
        }

        let winner_bids = self.get_winner_bids(&auction).await?;
        if winner_bids.is_empty() {
            join_all(auction.bids.into_iter().map(|bid| {
                self.update_bid_status(UpdateBidStatusInput {
                    bid,
                    new_status: T::BidStatusType::new_lost(),
                })
            }))
            .await;
            return Ok(());
        }

        let auction = self.repo.add_auction(auction).await?;
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
                tracing::debug!("Submitted transaction: {:?}", tx_hash);
                let auction = self.repo.submit_auction(auction, tx_hash.clone()).await?;
                join_all(auction.bids.iter().map(|bid| {
                    self.update_bid_status(UpdateBidStatusInput {
                        new_status: Service::get_new_status(
                            bid,
                            &winner_bids,
                            entities::BidStatusAuction {
                                tx_hash: tx_hash.clone(),
                                id:      auction.id,
                            },
                        ),
                        bid:        bid.clone(),
                    })
                }))
                .await;
            }
            Err(err) => {
                tracing::error!("Transaction failed to submit: {:?}", err);
            }
        };
        Ok(())
    }

    async fn submit_auction_for_lock(
        &self,
        permission_key: &entities::PermissionKey<T>,
        auction_lock: entities::AuctionLock,
    ) -> anyhow::Result<()> {
        let acquired_lock = auction_lock.lock().await;

        let bid_collection_time = OffsetDateTime::now_utc();
        let bids = self
            .repo
            .get_in_memory_bids_by_permission_key(permission_key)
            .await;

        match entities::Auction::try_new(bids, bid_collection_time) {
            Some(auction) => self.submit_auction(auction, acquired_lock).await,
            None => Ok(()),
        }
    }

    pub async fn handle_auction(&self, input: HandleAuctionInput<T>) -> anyhow::Result<()> {
        tracing::info!(
            chain_id = self.0.config.chain_id,
            permission_key = ?input.permission_key, // TODO: use string version of this
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
                let bids: Vec<entities::Bid<T>> = self
                    .repo
                    .get_in_memory_bids_by_permission_key(&permission_key)
                    .await
                    .into_iter()
                    .filter(|bid| bid.status.is_pending())
                    .collect();
                join_all(bids.into_iter().map(|bid| {
                    self.update_bid_status(UpdateBidStatusInput {
                        bid,
                        new_status: T::BidStatusType::new_lost(),
                    })
                }))
                .await;
                Ok(())
            }
        }
    }
}
