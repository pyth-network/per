use {
    super::{
        ChainType,
        Service,
    },
    crate::{
        api::RestError,
        opportunity::repository::InMemoryStore,
    },
    express_relay_api_types::opportunity::{
        GetOpportunitiesQueryParams,
        OpportunityId,
        OpportunityMode,
    },
};

pub struct GetOpportunitiesInput {
    pub query_params: GetOpportunitiesQueryParams,
}

pub struct GetLiveOpportunityByIdInput {
    pub opportunity_id: OpportunityId,
}

impl<T: ChainType> Service<T> {
    pub async fn get_live_opportunity_by_id(
        &self,
        input: GetLiveOpportunityByIdInput,
    ) -> Option<<T::InMemoryStore as InMemoryStore>::Opportunity> {
        self.repo
            .get_in_memory_opportunity_by_id(input.opportunity_id)
            .await
    }
    pub async fn get_opportunities(
        &self,
        input: GetOpportunitiesInput,
    ) -> Result<Vec<<T::InMemoryStore as InMemoryStore>::Opportunity>, RestError> {
        let query_params = input.query_params;
        if let Some(chain_id) = query_params.chain_id.clone() {
            self.get_config(&chain_id)?;
        }

        match query_params.mode.clone() {
            OpportunityMode::Live => Ok(self
                .repo
                .get_in_memory_opportunities()
                .await
                .values()
                .map(|opportunities| {
                    let opportunity = opportunities
                        .last()
                        .expect("An opportunity key vector should have at least one opportunity");
                    opportunity.clone()
                })
                .filter(|opportunity| {
                    let filter_time = if let Some(from_time) = query_params.from_time {
                        opportunity.creation_time >= from_time
                    } else {
                        true
                    };

                    let filter_chain_id = if let Some(chain_id) = &query_params.chain_id {
                        opportunity.chain_id == *chain_id
                    } else {
                        true
                    };
                    filter_time && filter_chain_id
                })
                .collect()),
            OpportunityMode::Historical => {
                let chain_id = query_params.chain_id.clone().ok_or_else(|| {
                    RestError::BadParameters("Chain id is required on historical mode".to_string())
                })?;
                self.repo
                    .get_opportunities(
                        &self.db,
                        chain_id,
                        query_params.permission_key.clone(),
                        query_params.from_time,
                    )
                    .await
            }
        }
    }
}
