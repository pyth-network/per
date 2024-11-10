use {
    super::Service,
    crate::{
        api::RestError,
        bid::entities,
        kernel::entities::{
            Evm,
            Svm,
        },
    },
    axum::async_trait,
};

pub struct VerifyBidInput<T: entities::BidCreateTrait> {
    pub bid_create: entities::BidCreate<T>,
}

#[async_trait]
pub trait Verification<T: entities::BidCreateTrait> {
    fn verify_bid(&self, input: VerifyBidInput<T>) -> Result<(), RestError>;
}

impl Verification<Evm> for Service<Evm> {
    // As we submit bids together for an auction, the bid is limited as follows:
    // 1. The bid amount should cover gas fees for all bids included in the submission.
    // 2. Depending on the maximum number of bids in the auction, the transaction size for the bid is limited.
    // 3. Depending on the maximum number of bids in the auction, the gas consumption for the bid is limited.
    fn verify_bid(&self, _input: VerifyBidInput<Evm>) -> Result<(), RestError> {
        todo!()
    }
}

impl Verification<Svm> for Service<Svm> {
    fn verify_bid(&self, _input: VerifyBidInput<Svm>) -> Result<(), RestError> {
        todo!();
    }
}
