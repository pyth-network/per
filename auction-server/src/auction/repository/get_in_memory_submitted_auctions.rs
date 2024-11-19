use {
    super::Repository,
    crate::auction::{
        entities,
        service::ChainTrait,
    },
};

impl<T: ChainTrait> Repository<T> {
    pub async fn get_in_memory_submitted_auctions(&self) -> Vec<entities::Auction<T>> {
        self.in_memory_store.submitted_auctions.read().await.clone()
    }
}
