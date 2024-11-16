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
    #[msg("Insufficient searcher funds")]
    InsufficientSearcherFunds,
    #[msg("Insufficient funds for rent")]
    InsufficientRent,
    #[msg("Invalid referral fee")]
    InvalidReferralFee,
    #[msg("Invalid ATA provided")]
    InvalidAta,
}
