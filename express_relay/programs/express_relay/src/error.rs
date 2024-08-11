use anchor_lang::prelude::*;

#[error_code]
pub enum ExpressRelayError {
    #[msg("Invalid fee splits")]
    InvalidFeeSplits,
    #[msg("Fees too high")]
    FeesTooHigh,
    #[msg("Deadline passed")]
    DeadlinePassed,
    #[msg("Invalid CPI into permission instruction")]
    InvalidCPIPermission,
    #[msg("Invalid permissioning")]
    InvalidPermissioning,
    #[msg("Insufficient Funds")]
    InsufficientFunds,
}
