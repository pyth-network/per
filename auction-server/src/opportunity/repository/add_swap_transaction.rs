use {
    super::Repository,
    crate::{
        api::RestError,
        auction::entities::Bid,
        kernel::entities::Svm,
        opportunity::{
            entities::OpportunityId,
            repository::InMemoryStoreSvm,
        },
    },
};

impl Repository<InMemoryStoreSvm> {
    pub async fn add_swap_bid(
        &self,
        opportunity_id: OpportunityId,
        bid: Bid<Svm>,
    ) -> Result<(), RestError> {
        self.in_memory_store
            .swap_bids
            .write()
            .await
            .insert(opportunity_id, bid);

        Ok(())
    }

    pub async fn get_swap_transaction(
        &self,
        opportunity_id: OpportunityId,
    ) -> Result<Option<Bid<Svm>>, RestError> {
        Ok(self
            .in_memory_store
            .swap_bids
            .read()
            .await
            .get(&opportunity_id)
            .cloned())
    }
}
