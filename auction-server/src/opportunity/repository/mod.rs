use {
    super::entities::opportunity::Opportunity,
    crate::kernel::entities::PermissionKey,
    std::collections::HashMap,
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
