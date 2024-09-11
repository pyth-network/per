use {
    dummy::SEED_ACCOUNTING,
    solana_sdk::pubkey::Pubkey,
};

pub fn get_accounting_key() -> Pubkey {
    Pubkey::find_program_address(&[SEED_ACCOUNTING], &dummy::id()).0
}
