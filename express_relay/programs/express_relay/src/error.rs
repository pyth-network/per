use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("Fee split(s) larger than fee precision")]
    FeeSplitLargerThanPrecision,
    #[msg("Fees higher than bid")]
    FeesHigherThanBid,
    #[msg("Deadline passed")]
    DeadlinePassed,
    #[msg("Invalid CPI into permission instruction")]
    InvalidCPIPermission,
    #[msg("Invalid permissioning")]
    InvalidPermissioning,
    #[msg("Insufficient Withdrawal Funds")]
    InsufficientWithdrawalFunds,
    #[msg("Insufficient Searcher Funds")]
    InsufficientSearcherFunds,
    #[msg("Insufficient protocol fee receiver funds for rent")]
    InsufficientProtocolFeeReceiverFundsForRent,
    #[msg("Insufficient relayer fee receiver funds for rent")]
    InsufficientRelayerFeeReceiverFundsForRent,
}
