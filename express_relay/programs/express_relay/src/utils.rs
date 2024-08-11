use anchor_lang::prelude::*;
use crate::{
    error::ExpressRelayError,
    state::*,
};

pub fn validate_fee_split(split: u64) -> Result<()> {
    if split > FEE_SPLIT_PRECISION {
        return err!(ExpressRelayError::InvalidFeeSplits);
    }
    Ok(())
}

pub fn transfer_lamports(
    from: &AccountInfo,
    to: &AccountInfo,
    amount: u64,
) -> Result<()> {
    **from.try_borrow_mut_lamports()? -= amount;
    **to.try_borrow_mut_lamports()? += amount;
    Ok(())
}
