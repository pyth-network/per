use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("Fee split(s) larger than fee precision")]
    FeeSplitLargerThanPrecision,
    #[msg("Fees higher than bid")]
    FeesHigherThanBid,
    #[msg("Deadline passed")]
    DeadlinePassed,
    #[msg("Invalid CPI into submit bid instruction")]
    InvalidCPISubmitBid,
    #[msg("Missing permission")]
    MissingPermission,
    #[msg("Multiple permissions")]
    MultiplePermissions,
    #[msg("Insufficient Searcher Funds")]
    InsufficientSearcherFunds,
    #[msg("Insufficient protocol fee receiver funds for rent")]
    InsufficientProtocolFeeReceiverRent,
    #[msg("Insufficient relayer fee receiver funds for rent")]
    InsufficientRelayerFeeReceiverRent,
    #[msg("Invalid PDA provided")]
    InvalidPDAProvided,
}
