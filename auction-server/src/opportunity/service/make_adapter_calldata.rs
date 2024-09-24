use {
    super::{
        make_opportunity_execution_params::MakeOpportunityExecutionParamsInput,
        ChainTypeEvm,
        Service,
    },
    crate::{
        api::{
            Auth,
            RestError,
        },
        opportunity::{
            api::{
                OpportunityBid,
                OpportunityId,
            },
            contracts::OpportunityAdapter,
            entities::{
                opportunity::Opportunity,
                opportunity_evm::OpportunityEvm,
            },
        },
    },
    ethers::types::Bytes,
    std::sync::Arc,
    time::OffsetDateTime,
    uuid::Uuid,
};

pub struct MakeAdapterCalldataInput {
    pub opportunity:     OpportunityEvm,
    pub opportunity_bid: OpportunityBid,
}

impl Service<ChainTypeEvm> {
    pub async fn make_adapter_calldata(
        &self,
        input: MakeAdapterCalldataInput,
    ) -> Result<Bytes, RestError> {
        let config = self.get_config(&input.opportunity.chain_id)?;
        let adapter_contract = config.adapter_factory_contract;
        let signature = input.opportunity_bid.signature.clone();
        let execution_params =
            self.make_opportunity_execution_params(MakeOpportunityExecutionParamsInput {
                opportunity: input.opportunity,

                opportunity_bid: input.opportunity_bid,
            })?;

        let client = Arc::new(config.provider.clone());
        let calldata = OpportunityAdapter::new(adapter_contract, client.clone())
            .execute_opportunity(execution_params, signature.to_vec().into())
            .calldata()
            .ok_or(RestError::BadParameters(
                "Failed to generate calldata for opportunity adapter".to_string(),
            ))?;

        Ok(calldata)
    }
}
