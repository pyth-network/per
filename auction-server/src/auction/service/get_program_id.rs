use {
    super::Service,
    crate::kernel::entities::Svm,
    solana_sdk::pubkey::Pubkey,
};

impl Service<Svm> {
    pub fn get_program_id(&self) -> Pubkey {
        self.config.chain_config.express_relay.program_id
    }
}
