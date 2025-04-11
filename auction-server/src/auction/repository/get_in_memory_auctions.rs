use {
    super::Repository,
    crate::auction::entities,
};

impl Repository {
    pub async fn get_in_memory_auctions(&self) -> Vec<entities::Auction> {
        self.in_memory_store
            .auctions
            .read()
            .await
            .values()
            .cloned()
            .collect()
    }
}
