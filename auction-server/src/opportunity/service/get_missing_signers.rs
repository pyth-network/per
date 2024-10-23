use {
    super::{
        ChainTypeSvm,
        Service,
    },
    crate::opportunity::entities,
    solana_sdk::pubkey::Pubkey,
};

impl Service<ChainTypeSvm> {
    pub fn get_missing_signers(&self, opportunity: entities::OpportunitySvm) -> Vec<Pubkey> {
        match opportunity.program {
            entities::OpportunitySvmProgram::Phantom(data) => vec![data.user_wallet_address],
            _ => vec![],
        }
    }
}
