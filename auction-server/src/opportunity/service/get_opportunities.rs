use {
    super::{
        make_adapter_calldata::MakeAdapterCalldataInput,
        verify_opportunity::VerifyOpportunityInput,
        ChainTypeEvm,
        Service,
    },
    crate::{
        api::{
            ws::UpdateEvent::NewOpportunity,
            Auth,
            RestError,
        },
        auction::{
            handle_bid,
            BidEvm,
        },
        kernel::entities::ChainId,
        opportunity::{
            api::{
                GetOpportunitiesQueryParams,
                OpportunityBid,
                OpportunityId,
                OpportunityMode,
                OpportunityParamsWithMetadata,
            },
            contracts::{
                erc20,
                OpportunityAdapterErrors,
            },
            entities,
        },
    },
    ethers::{
        contract::ContractRevert,
        types::Bytes,
    },
    time::OffsetDateTime,
    uuid::Uuid,
};

pub struct GetOpportunitiesInput {
    pub query_params: GetOpportunitiesQueryParams,
}

impl Service<ChainTypeEvm> {
    pub async fn get_opportunities(
        &self,
        input: GetOpportunitiesInput,
    ) -> Result<Vec<entities::OpportunityEvm>, RestError> {
        let query_params = input.query_params;
        if let Some(chain_id) = query_params.chain_id.clone() {
            self.get_config(&chain_id)?;
        }

        match query_params.mode.clone() {
            OpportunityMode::Live => {
                Ok(self
                    .repo
                    .get_opportunities()
                    .await
                    .iter()
                    .map(|(_key, opportunities)| {
                        let opportunity = opportunities
                            .last()
                            .expect("A permission key vector should have at least one opportunity");
                        opportunity.clone()
                    })
                    .filter(|opportunity| {
                        // let OpportunityParams::V1(params) = &params_with_id.params;
                        if let Some(chain_id) = &query_params.chain_id {
                            opportunity.chain_id == *chain_id
                        } else {
                            true
                        }
                    })
                    .collect())
            }
            OpportunityMode::Historical => {
                let chain_id = query_params.chain_id.clone().ok_or_else(|| {
                    RestError::BadParameters("Chain id is required on historical mode".to_string())
                })?;
                self.repo
                    .get_opportunities_by_permission_key(
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
