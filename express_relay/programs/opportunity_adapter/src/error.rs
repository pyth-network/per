use anchor_lang::prelude::*;

#[error_code]
pub enum OpportunityAdapterError {
    #[msg("No token checking")]
    NoTokenChecking,
    #[msg("Token expectation not met")]
    TokenExpectationNotMet,
    #[msg("Signature expired")]
    SignatureExpired,
    #[msg("Signature verification failed")]
    SignatureVerificationFailed,
}
