use {
    super::{
        ChainTypeSvm,
        Service,
    },
    crate::api::RestError,
    solana_sdk::pubkey::Pubkey,
};

pub struct EstimatePriceInput {
    pub input_token_amount:          u64,
    pub input_token_mint:            Pubkey,
    pub output_token_mint:           Pubkey,
    pub maximum_slippage_percentage: f64,
}

impl Service<ChainTypeSvm> {
    pub async fn estimate_price(&self, _input: EstimatePriceInput) -> Result<u64, RestError> {
        // TODO implement
        return Ok(0);
    }
}
