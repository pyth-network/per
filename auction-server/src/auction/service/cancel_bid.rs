use {
    super::{
        update_bid_status::UpdateBidStatusInput,
        Service,
    },
    crate::{
        api::RestError,
        auction::entities,
        kernel::entities::Svm,
        models::Profile,
    },
};

#[derive(Debug, Clone)]
pub struct CancelBidInput {
    pub bid_id:  entities::BidId,
    pub profile: Profile,
}

impl Service<Svm> {
    #[tracing::instrument(skip_all, err)]
    async fn cancel_bid_for_lock(
        &self,
        input: CancelBidInput,
        lock: entities::BidLock,
    ) -> Result<(), RestError> {
        let _lock = lock.lock().await;
        let bid = self
            .repo
            .get_in_memory_auction_bid_by_bid_id(input.bid_id)
            .await
            .ok_or(RestError::BadParameters(
                "Bid is only cancellable in awaiting_signature state".to_string(),
            ))?;

        if bid.profile_id.ok_or(RestError::Forbidden)? != input.profile.id {
            return Err(RestError::Forbidden);
        }

        match bid.status.clone() {
            entities::BidStatusSvm::AwaitingSignature { auction } => {
                let tx_hash = bid.chain_data.transaction.signatures[0];
                self.update_bid_status(UpdateBidStatusInput {
                    bid,
                    new_status: entities::BidStatusSvm::Cancelled {
                        auction: entities::BidStatusAuction {
                            id: auction.id,
                            tx_hash,
                        },
                    },
                })
                .await?;
                Ok(())
            }
            _ => Err(RestError::BadParameters(
                "Bid is only cancellable in awaiting_signature state".to_string(),
            )),
        }
    }

    #[tracing::instrument(skip_all, err, fields(bid_id = %input.bid_id))]
    pub async fn cancel_bid(&self, input: CancelBidInput) -> Result<(), RestError> {
        // Lock the bid to prevent submission
        let bid_lock = self
            .repo
            .get_or_create_in_memory_bid_lock(input.bid_id)
            .await;
        let result = self.cancel_bid_for_lock(input.clone(), bid_lock).await;
        self.repo.remove_in_memory_bid_lock(&input.bid_id).await;
        result
    }
}
