use {
    super::Repository,
    crate::auction::entities,
};

impl Repository {
    pub fn get_in_memory_auctions(&self) -> Vec<entities::Auction> {
        self.in_memory_store
            .auctions
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }
}
