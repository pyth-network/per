use {
    super::{
        make_permitted_tokens::MakePermittedTokensInput,
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
            contracts::{
                ExecutionParams,
                ExecutionWitness,
                PermitBatchTransferFrom,
                TokenAmount,
            },
            entities::{
                opportunity::Opportunity,
                opportunity_evm::OpportunityEvm,
            },
        },
    },
    ethers::{
        contract::abigen,
        utils::get_create2_address_from_hash,
    },
    time::OffsetDateTime,
    uuid::Uuid,
};

pub struct MakeOpportunityExecutionParamsInput {
    pub opportunity:     OpportunityEvm,
    pub opportunity_bid: OpportunityBid,
}

impl Service<ChainTypeEvm> {
    pub fn make_opportunity_execution_params(
        &self,
        input: MakeOpportunityExecutionParamsInput,
    ) -> Result<ExecutionParams, RestError> {
        Ok(ExecutionParams {
            permit:  PermitBatchTransferFrom {
                permitted: self.make_permitted_tokens(MakePermittedTokensInput {
                    opportunity:     input.opportunity.clone(),
                    opportunity_bid: input.opportunity_bid.clone(),
                })?,
                nonce:     input.opportunity_bid.nonce,
                deadline:  input.opportunity_bid.deadline,
            },
            witness: ExecutionWitness {
                buy_tokens:        input
                    .opportunity
                    .buy_tokens
                    .clone()
                    .into_iter()
                    .map(|token| TokenAmount {
                        token:  token.token,
                        amount: token.amount,
                    })
                    .collect(),
                executor:          input.opportunity_bid.executor,
                target_contract:   input.opportunity.target_contract,
                target_calldata:   input.opportunity.target_calldata,
                target_call_value: input.opportunity.target_call_value,
                bid_amount:        input.opportunity_bid.amount,
            },
        })
    }
}
