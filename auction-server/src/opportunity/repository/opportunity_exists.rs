use super::{
    Cache,
    Repository,
};

impl<T: Cache> Repository<T> {
    pub async fn opportunity_exists(&self, opportunity: &T::Opportunity) -> bool {
        self.cache
            .opportunities
            .read()
            .await
            .get(&opportunity.permission_key)
            .map_or(false, |opps| opps.contains(opportunity))
    }
}
