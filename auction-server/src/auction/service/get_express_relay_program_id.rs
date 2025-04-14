use {
    super::Service,
    solana_sdk::pubkey::Pubkey,
};

impl Service {
    pub fn get_express_relay_program_id(&self) -> Pubkey {
        self.config.chain_config.express_relay.program_id
    }
}
