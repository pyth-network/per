use {
    super::Repository,
    crate::auction::{
        entities,
        service::ChainTrait,
    },
};

impl<T: ChainTrait> Repository<T> {
    #[tracing::instrument(skip_all, fields(auction_id))]
    pub async fn remove_in_memory_auction(&self, auction_id: entities::AuctionId) {
        tracing::Span::current().record("auction_id", auction_id.to_string());
        let mut write_guard = self.in_memory_store.auctions.write().await;
        write_guard.retain(|a| a.id != auction_id);
    }
}
