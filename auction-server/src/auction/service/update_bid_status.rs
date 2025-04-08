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
        auction::entities,
    },
    express_relay_api_types::bid::BidStatusWithId,
};

pub struct UpdateBidStatusInput<T: ChainTrait> {
    pub bid:        entities::Bid<T>,
    pub new_status: T::BidStatusType,
}

impl<T: ChainTrait> Service<T> {
    #[tracing::instrument(skip_all, fields(bid_id, status), err(level = tracing::Level::TRACE))]
    pub async fn update_bid_status(
        &self,
        input: UpdateBidStatusInput<T>,
    ) -> Result<bool, RestError> {
        tracing::Span::current().record("bid_id", input.bid.id.to_string());
        tracing::Span::current().record("status", format!("{:?}", input.new_status));

        let is_updated = self
            .repo
            .update_bid_status(input.bid.clone(), input.new_status.clone())
            .await
            .map_err(|e| {
                tracing::error!(error = ?e, "Failed to update bid status");
                RestError::TemporarilyUnavailable
            })?;

        // TODO: Do not rely on db to see if the status is changed
        // we can rely on the write guard and our in memory structure

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
        Ok(is_updated)
    }
}
