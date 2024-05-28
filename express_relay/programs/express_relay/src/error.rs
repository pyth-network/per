use anchor_lang::prelude::*;

#[error_code]
pub enum ExpressRelayError {
    // #[msg("Bid not met")]
    // BidNotMet,
    #[msg("Invalid fee splits")]
    InvalidFeeSplits,
    #[msg("Permissioning instructions out of order")]
    PermissioningOutOfOrder,
    #[msg("Relayer signer used elsewhere")]
    RelayerSignerUsedElsewhere,
    #[msg("Fees too high")]
    FeesTooHigh,
    #[msg("Signature expired")]
    SignatureExpired,
    #[msg("Signature verification failed")]
    SignatureVerificationFailed,
}
