use {
    super::entities::{
        opportunity::Opportunity,
        opportunity_evm::OpportunityEvm,
        opportunity_svm::OpportunitySvm,
    },
    crate::kernel::entities::PermissionKey,
    std::{
        collections::HashMap,
        sync::Arc,
    },
    tokio::sync::RwLock,
};

mod get_opportunities_by_permission_key_and_id;

#[derive(Debug)]
pub struct Repository<T: Opportunity> {
    opportunities: RwLock<HashMap<PermissionKey, Vec<T>>>,
}

impl<T: Opportunity> Repository<T> {
    pub fn new() -> Self {
        Self {
            opportunities: RwLock::new(HashMap::new()),
        }
    }
}
