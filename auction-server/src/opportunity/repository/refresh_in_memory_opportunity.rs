use {
    super::{
        InMemoryStore,
        OpportunityTable,
        Repository,
    },
    crate::opportunity::entities::Opportunity,
    std::collections::hash_map::Entry,
};

impl<T: InMemoryStore> Repository<T> {
    pub async fn refresh_in_memory_opportunity(
        &self,
        opportunity: T::Opportunity,
    ) -> T::Opportunity {
        let mut refreshed_opportunity = opportunity.clone();
        refreshed_opportunity.refresh();

        let key = opportunity.get_key();
        let mut write_guard = self.in_memory_store.opportunities.write().await;
        match write_guard.entry(key.clone()) {
            Entry::Occupied(mut entry) => {
                let opportunities = entry.get_mut();
                match opportunities.iter().position(|o| *o == opportunity) {
                    Some(index) => opportunities[index] = refreshed_opportunity.clone(),
                    None => {
                        tracing::error!(opportunity = ?opportunity, "Refresh opportunity failed, opportunity not found");
                    }
                }
            }
            Entry::Vacant(_) => {
                tracing::error!(key = ?key, opportunity = ?opportunity, "Refresh opportunity failed, entry not found");
            }
        }

        refreshed_opportunity
    }
}
