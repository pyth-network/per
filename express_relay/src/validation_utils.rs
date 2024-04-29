use solana_program::{
    entrypoint::ProgramResult,
    msg,
    pubkey::Pubkey,
};

use super::error::ExpressRelayError;

pub fn assert_keys_equal(key1: Pubkey, key2: Pubkey) -> ProgramResult {
    if key1 != key2 {
        msg!("Error: unexpected public key, validation utils");
        Err(ExpressRelayError::PublicKeyMismatch.into())
    } else {
        Ok(())
    }
}

pub fn validate_fee_splits(split_protocol: u64, split_relayer: u64, split_precision: u64) -> ProgramResult {
    if split_precision > 0 {
        if split_protocol <= split_precision {
            if split_relayer <= split_precision {
                return Ok(());
            }
        }
    }

    return Err(ExpressRelayError::InvalidFeeSplits.into())
}
