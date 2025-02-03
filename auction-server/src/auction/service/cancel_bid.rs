use {
    super::{
        update_bid_status::UpdateBidStatusInput,
        Service,
    },
    crate::{
        api::RestError,
        auction::entities::{
            self,
            BidStatus,
        },
        kernel::entities::Svm,
        models::Profile,
    },
};

pub struct CancelBidInput {
    pub bid_id:  entities::BidId,
    pub profile: Profile,
}

impl Service<Svm> {
    async fn cancel_auction_bid_for_lock(
        &self,
        bid: entities::Bid<Svm>,
        auction: entities::Auction<Svm>,
        _lock: entities::BidLock,
    ) -> Result<(), RestError> {
        if !bid.status.is_awaiting_signature() {
            return Err(RestError::BadParameters(
                "Bid is not cancellable".to_string(),
            ));
        }

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
        .await
    }

    pub async fn cancel_bid(&self, input: CancelBidInput) -> Result<(), RestError> {
        let (bid, auction) = self
            .repo
            .get_in_memory_auction_bid_by_bid_id(input.bid_id)
            .await
            .ok_or(RestError::BadParameters(
                "Bid is not cancellable".to_string(),
            ))?;
        if bid.profile_id.ok_or(RestError::Forbidden)? != input.profile.id {
            return Err(RestError::Forbidden);
        }

        // Lock the bid to prevent submission
        let bid_lock = self
            .repo
            .get_or_create_in_memory_bid_lock(input.bid_id)
            .await;
        let result = self
            .cancel_auction_bid_for_lock(bid, auction, bid_lock)
            .await;
        self.repo.remove_in_memory_bid_lock(&input.bid_id).await;
        result
    }
}
