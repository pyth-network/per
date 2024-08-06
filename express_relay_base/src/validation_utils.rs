use solana_program::{
    entrypoint::ProgramResult,
    msg,
    pubkey::Pubkey,
};

use crate::state::FEE_SPLIT_PRECISION;

use super::error::ExpressRelayError;

pub fn assert_keys_equal(key1: Pubkey, key2: Pubkey) -> ProgramResult {
    if key1 != key2 {
        msg!("Error: unexpected public key, validation utils");
        Err(ExpressRelayError::PublicKeyMismatch.into())
    } else {
        Ok(())
    }
}

pub fn validate_fee_split(split: u64) -> ProgramResult {
    if split > FEE_SPLIT_PRECISION {
        return Err(ExpressRelayError::InvalidFeeSplits.into())
    }
    Ok(())
}
