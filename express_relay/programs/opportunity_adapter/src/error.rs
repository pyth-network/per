use anchor_lang::prelude::*;

#[error_code]
pub enum OpportunityAdapterError {
    #[msg("Improper token checking")]
    ImproperTokenChecking,
    #[msg("Token expectation not met")]
    TokenExpectationNotMet,
}
