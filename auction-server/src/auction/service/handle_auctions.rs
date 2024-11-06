use {
    super::{
        auctionable::Auctionable,
        ChainTrait,
        Service,
    },
    crate::auction::service::handle_auction::HandleAuctionInput,
};

impl<T: ChainTrait> Service<T>
where
    Service<T>: Auctionable<T>,
{
    pub async fn handle_auctions(&self) {
        let permission_keys = self.get_permission_keys_for_auction().await;

        tracing::info!(
            chain_id = self.0.config.chain_id,
            auction_len = permission_keys.len(),
            "Handling auctions...",
        );

        for permission_key in permission_keys.into_iter() {
            self.task_tracker.spawn({
                let service = self.clone();
                async move {
                    let result = service
                        .handle_auction(HandleAuctionInput {
                            permission_key: permission_key.clone(),
                        })
                        .await;
                    if let Err(err) = result {
                        tracing::error!(
                            error = ?err,
                            chain_id = service.config.chain_id,
                            permission_key = ?permission_key,
                            "Failed to submit auction",
                        );
                    }
                }
            });
        }
    }
}
