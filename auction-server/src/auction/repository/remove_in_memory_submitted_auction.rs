use {
    super::Repository,
    crate::auction::{
        entities,
        service::ChainTrait,
    },
};

impl<T: ChainTrait> Repository<T> {
    #[tracing::instrument(skip_all, fields(auction_id))]
    pub async fn remove_in_memory_submitted_auction(&self, auction: entities::Auction<T>) {
        tracing::Span::current().record("auction_id", auction.id.to_string());
        let mut write_guard = self.in_memory_store.submitted_auctions.write().await;
        write_guard.retain(|a| a.id != auction.id);
    }
}
