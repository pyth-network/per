use {
    super::Repository,
    crate::auction::entities,
    solana_sdk::signature::Signature,
};

impl Repository {
    #[tracing::instrument(skip_all, name = "submit_auction_repo", fields(auction_id, tx_hash))]
    pub async fn submit_auction(
        &self,
        auction: entities::Auction,
        transaction_hash: Signature,
        winner_bid_ids: Vec<entities::BidId>,
    ) -> anyhow::Result<entities::Auction> {
        tracing::Span::current().record("auction_id", auction.id.to_string());
        tracing::Span::current().record("tx_hash", format!("{:?}", transaction_hash));

        if let Some(mut updated_auction) =
            self.db.submit_auction(&auction, &transaction_hash).await?
        {
            for bid in &mut updated_auction.bids {
                if winner_bid_ids.contains(&bid.id) {
                    bid.submission_time = updated_auction.submission_time;
                }
            }
            self.update_in_memory_auction(updated_auction.clone()).await;
            Ok(updated_auction)
        } else {
            Ok(auction)
        }
    }
}
