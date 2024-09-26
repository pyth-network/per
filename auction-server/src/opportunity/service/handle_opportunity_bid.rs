use {
    super::{
        make_adapter_calldata::MakeAdapterCalldataInput,
        ChainTypeEvm,
        Service,
    },
    crate::{
        api::{
            Auth,
            RestError,
        },
        auction::{
            handle_bid,
            BidEvm,
        },
        opportunity::{
            api::{
                OpportunityBidEvm,
                OpportunityId,
            },
            contracts::{
                erc20,
                OpportunityAdapterErrors,
            },
        },
    },
    ethers::{
        contract::ContractRevert,
        types::Bytes,
    },
    time::OffsetDateTime,
    uuid::Uuid,
};

pub struct HandleOpportunityBidInput {
    pub opportunity_id:  OpportunityId,
    pub opportunity_bid: OpportunityBidEvm,
    pub initiation_time: OffsetDateTime,
    pub auth:            Auth,
}

fn parse_revert_error(revert: &Bytes) -> Option<String> {
    let apdapter_decoded =
        OpportunityAdapterErrors::decode_with_selector(revert).map(|decoded_error| {
            format!(
                "Opportunity Adapter Contract Revert Error: {:#?}",
                decoded_error
            )
        });
    let erc20_decoded = erc20::ERC20Errors::decode_with_selector(revert)
        .map(|decoded_error| format!("ERC20 Contract Revert Error: {:#?}", decoded_error));
    apdapter_decoded.or(erc20_decoded)
}

impl Service<ChainTypeEvm> {
    pub async fn handle_opportunity_bid(
        &self,
        input: HandleOpportunityBidInput,
    ) -> Result<Uuid, RestError> {
        let opportunity = self
            .repo
            .get_opportunities_by_permission_key_and_id(
                input.opportunity_id,
                &input.opportunity_bid.permission_key,
            )
            .await
            .ok_or(RestError::OpportunityNotFound)?;
        let config = self.get_config(&opportunity.chain_id)?;

        let adapter_calldata = self
            .make_adapter_calldata(MakeAdapterCalldataInput {
                opportunity:     opportunity.clone(),
                opportunity_bid: input.opportunity_bid.clone(),
            })
            .await
            .map_err(|e| {
                tracing::error!(
                    "Error making adapter calldata: {:?} - opportunity: {:?}",
                    e,
                    opportunity
                );
                e
            })?;
        let bid = BidEvm {
            permission_key:  input.opportunity_bid.permission_key.clone(),
            chain_id:        opportunity.chain_id.clone(),
            target_contract: config.adapter_factory_contract,
            target_calldata: adapter_calldata,
            amount:          input.opportunity_bid.amount,
        };
        match handle_bid(
            self.store.clone(),
            bid.clone(),
            input.initiation_time,
            input.auth,
        )
        .await
        {
            Ok(id) => Ok(id),
            Err(e) => {
                tracing::warn!(
                    "Error handling bid: {:?} - opportunity: {:?} - bid: {:?}",
                    e,
                    opportunity,
                    bid
                );
                match e {
                    RestError::SimulationError { result, reason } => {
                        let parsed = parse_revert_error(&result);
                        match parsed {
                            Some(decoded) => Err(RestError::BadParameters(decoded)),
                            None => {
                                tracing::info!("Could not parse revert reason: {}", reason);
                                Err(RestError::SimulationError { result, reason })
                            }
                        }
                    }
                    _ => Err(e),
                }
            }
        }
    }
}
