use {
    super::Repository,
    crate::auction::{
        entities,
        service::ChainTrait,
    },
};

impl<T: ChainTrait> Repository<T> {
    #[tracing::instrument(skip_all, fields(auction_id))]
    pub async fn update_in_memory_auction(&self, auction: entities::Auction<T>) {
        tracing::Span::current().record("auction_id", auction.id.to_string());
        let mut write_gaurd = self.in_memory_store.auctions.write().await;
        let updated = match write_gaurd.iter_mut().find(|a| a.id == auction.id) {
            Some(a) => {
                *a = auction.clone();
                true
            }
            None => {
                tracing::error!(auction = ?auction, "Auction not found in in-memory store");
                false
            }
        };
        if !updated {
            write_gaurd.push(auction.clone());
        }
    }
}
