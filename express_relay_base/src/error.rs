use solana_program::program_error::ProgramError;
use thiserror::Error;

#[derive(Error, Debug, Copy, Clone)]
pub enum ExpressRelayError {
    #[error("Public key does not match expectation")]
    PublicKeyMismatch,
    #[error("Bid not met")]
    BidNotMet,
    #[error("Invalid fee splits")]
    InvalidFeeSplits,
    #[error("Permission already toggled")]
    PermissionAlreadyToggled,
    #[error("Already initialized")]
    AlreadyInitialized,
    #[error("Permissioning instructions out of order")]
    PermissioningOutOfOrder,
    #[error("Relayer signer used elsewhere")]
    RelayerSignerUsedElsewhere,
}

impl From<ExpressRelayError> for ProgramError {
    fn from(e: ExpressRelayError) -> Self {
        ProgramError::Custom(e as u32)
    }
}
