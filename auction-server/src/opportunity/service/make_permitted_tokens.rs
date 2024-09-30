use {
    super::{
        ChainTypeEvm,
        Service,
    },
    crate::{
        api::RestError,
        opportunity::{
            api::OpportunityBidEvm,
            contracts::TokenPermissions,
            entities,
        },
    },
    ethers::types::U256,
};

pub struct MakePermittedTokensInput {
    pub opportunity:     entities::OpportunityCreateEvm,
    pub opportunity_bid: OpportunityBidEvm,
}

impl Service<ChainTypeEvm> {
    pub(super) fn make_permitted_tokens(
        &self,
        input: MakePermittedTokensInput,
    ) -> Result<Vec<TokenPermissions>, RestError> {
        let config = self.get_config(&input.opportunity.core_fields.chain_id)?;
        let mut permitted_tokens: Vec<TokenPermissions> = input
            .opportunity
            .core_fields
            .sell_tokens
            .clone()
            .into_iter()
            .map(|token| TokenPermissions {
                token:  token.token,
                amount: token.amount,
            })
            .collect();

        let extra_weth_amount = input.opportunity_bid.amount + input.opportunity.target_call_value;
        if let Some(weth_position) = permitted_tokens.iter().position(|x| x.token == config.weth) {
            permitted_tokens[weth_position] = TokenPermissions {
                amount: permitted_tokens[weth_position].amount + extra_weth_amount,
                ..permitted_tokens[weth_position]
            }
        } else if extra_weth_amount > U256::zero() {
            permitted_tokens.push(TokenPermissions {
                token:  config.weth,
                amount: extra_weth_amount,
            });
        }
        Ok(permitted_tokens)
    }
}
