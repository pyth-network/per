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
            entities::{
                BidChainDataCreateEvm,
                BidCreate,
            },
            service::handle_bid::HandleBidInput,
        },
        kernel::{
            contracts::{
                erc20,
                OpportunityAdapterErrors,
            },
            entities::Evm,
        },
    },
    ethers::{
        contract::ContractRevert,
        types::Bytes,
    },
    express_relay_api_types::opportunity::{
        OpportunityBidEvm,
        OpportunityId,
    },
    time::OffsetDateTime,
    uuid::Uuid,
};

pub struct HandleOpportunityBidInput {
    pub opportunity_id:  OpportunityId,
    pub opportunity_bid: OpportunityBidEvm,
    #[allow(dead_code)]
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
            .get_in_memory_opportunity_by_id(input.opportunity_id)
            .await
            .ok_or(RestError::OpportunityNotFound)?;

        let config = self.get_config(&opportunity.chain_id)?;
        let auction_service = config.get_auction_service().await;
        let adapter_calldata = self
            .make_adapter_calldata(MakeAdapterCalldataInput {
                opportunity:     opportunity.clone().into(),
                opportunity_bid: input.opportunity_bid.clone(),
            })
            .map_err(|e| {
                tracing::error!(
                    "Error making adapter calldata: {:?} - opportunity: {:?}",
                    e,
                    opportunity
                );
                e
            })?;

        let profile = match input.auth {
            Auth::Authorized(_, profile) => Some(profile),
            Auth::Admin => None,
            Auth::Unauthorized => None,
        };

        let bid_create = BidCreate::<Evm> {
            chain_id: opportunity.chain_id.clone(),
            initiation_time: OffsetDateTime::now_utc(),
            profile,
            chain_data: BidChainDataCreateEvm {
                target_contract: config.adapter_factory_contract,
                target_calldata: adapter_calldata,
                permission_key:  input.opportunity_bid.permission_key.clone(),
                amount:          input.opportunity_bid.amount,
            },
        };
        match auction_service
            .handle_bid(HandleBidInput {
                bid_create: bid_create.clone(),
            })
            .await
        {
            Ok(bid) => Ok(bid.id),
            Err(e) => {
                tracing::warn!(
                    error = ?e,
                    opportunity = ?opportunity,
                    bid_create = ?bid_create,
                    "Handling bid failed for opportunity_bid",
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
