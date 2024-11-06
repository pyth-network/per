use {
    super::{
        ChainTrait,
        Service,
    },
    crate::{
        api::{
            ws::UpdateEvent,
            RestError,
        },
        auction::{
            api::BidStatusWithId,
            entities,
        },
    },
};

pub struct UpdateBidStatusInput<T: ChainTrait> {
    pub bid:        entities::Bid<T>,
    pub new_status: T::BidStatusType,
}

impl<T: ChainTrait> Service<T> {
    pub async fn update_bid_status(&self, input: UpdateBidStatusInput<T>) -> Result<(), RestError> {
        let is_updated = self
            .repo
            .update_bid_status(input.bid.clone(), input.new_status.clone())
            .await
            .map_err(|e| {
                tracing::error!(error = ?e, "Failed to update bid status");
                RestError::TemporarilyUnavailable
            })?;

        // It is possible to call this function multiple times from different threads if receipts are delayed
        // Or the new block is mined faster than the bid status is updated.
        // To ensure we do not broadcast the update more than once, we need to check the below "if"
        if is_updated {
            // TODO remove this line and move BidStatusWithId somewhere else
            if let Err(e) = self
                .event_sender
                .send(UpdateEvent::BidStatusUpdate(BidStatusWithId {
                    id:         input.bid.id,
                    bid_status: input.new_status.into(),
                }))
            {
                tracing::error!(error = e.to_string(), "Failed to send update event");
            }
        }
        Ok(())
    }
}
