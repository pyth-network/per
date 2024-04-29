use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::pubkey::Pubkey;
use std::mem::size_of;

pub const RESERVE_PERMISSION: usize = 200;
pub const SEED_PERMISSION: &[u8] = b"permission";

#[repr(C)]
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct PermissionMetadata {
    pub bump: u8,
    pub balance: u64,
    pub bid_amount: u64,
}

impl PermissionMetadata {
    pub const LEN: usize = size_of::<u8>() + 32*size_of::<u8>() + size_of::<u64>() + RESERVE_PERMISSION;
}

pub const RESERVE_EXPRESS_RELAY_METADATA: usize = 200;
pub const SEED_METADATA: &[u8] = b"metadata";

#[repr(C)]
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct ExpressRelayMetadata {
    pub bump: u8,
    pub admin: Pubkey,
    pub relayer_signer: Pubkey,
    pub relayer_fee_receiver: Pubkey,
    pub split_protocol: u64,
    pub split_relayer: u64,
    pub split_precision: u64,
}
