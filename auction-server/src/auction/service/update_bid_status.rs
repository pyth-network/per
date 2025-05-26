use {
    super::{
        get_bid_transaction_data::GetBidTransactionDataInput,
        Service,
    },
    crate::{
        api::{
            ws::UpdateEvent,
            RestError,
        },
        auction::entities,
        opportunity::service::get_token_mint::GetTokenMintInput,
    },
    express_relay_api_types::bid::BidStatusWithId,
    std::collections::HashMap,
};

pub struct UpdateBidStatusInput {
    pub bid:        entities::Bid,
    pub new_status: entities::BidStatusSvm,
}

impl Service {
    async fn add_bid_analytics(&self, bid: entities::Bid) -> Result<(), RestError> {
        let transaction_data = self
            .get_bid_transaction_data(GetBidTransactionDataInput { bid: bid.clone() })
            .await?;
        let decimals = match transaction_data.clone() {
            entities::BidTransactionData::SubmitBid(_) => HashMap::new(),
            entities::BidTransactionData::Swap(data) => {
                let searcher_mint = self
                    .opportunity_service
                    .get_token_mint(GetTokenMintInput {
                        chain_id: self.config.chain_id.clone(),
                        mint:     data.accounts.mint_searcher,
                    })
                    .await?;

                let user_mint = self
                    .opportunity_service
                    .get_token_mint(GetTokenMintInput {
                        chain_id: self.config.chain_id.clone(),
                        mint:     data.accounts.mint_user,
                    })
                    .await?;

                HashMap::from([
                    (data.accounts.mint_searcher, searcher_mint.decimals),
                    (data.accounts.mint_user, user_mint.decimals),
                ])
            }
        };
        self.repo
            .add_bid_analytics(
                bid.clone(),
                transaction_data,
                self.store.prices.read().await.clone(),
                decimals,
            )
            .await
            .map_err(|e| {
                tracing::error!(error = ?e, "Failed to add bid analytics");
                RestError::TemporarilyUnavailable
            })
    }

    #[tracing::instrument(skip_all, fields(bid_id, status), err(level = tracing::Level::TRACE))]
    pub async fn update_bid_status(&self, input: UpdateBidStatusInput) -> Result<bool, RestError> {
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
            self.task_tracker.spawn({
                let (service, mut bid) = (self.clone(), input.bid.clone());
                bid.status = input.new_status.clone();
                async move {
                    if let Err(e) = service.add_bid_analytics(bid.clone()).await {
                        tracing::error!(bid = ?bid, error = ?e, "Failed to add bid analytics");
                    }
                }
            });

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
