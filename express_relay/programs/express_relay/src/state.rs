use anchor_lang::prelude::*;

pub const FEE_SPLIT_PRECISION: u64 = 1_000_000_000_000_000_000;

pub const RESERVE_PERMISSION: usize = 200;
pub const SEED_PERMISSION: &[u8] = b"permission";

#[account]
#[derive(Default)]
pub struct PermissionMetadata {
    pub bump: u8,
    pub balance: u64,
    pub bid_amount: u64,
}

pub const RESERVE_EXPRESS_RELAY_METADATA: usize = 200;
pub const SEED_METADATA: &[u8] = b"metadata";

#[account]
#[derive(Default)]
pub struct ExpressRelayMetadata {
    pub bump: u8,
    pub admin: Pubkey,
    pub relayer_signer: Pubkey,
    pub relayer_fee_receiver: Pubkey,
    pub split_protocol_default: u64,
    pub split_relayer: u64,
}

pub const RESERVE_EXPRESS_RELAY_CONFIG_PROTOCOL: usize = 200;
pub const SEED_CONFIG_PROTOCOL: &[u8] = b"config_protocol";

#[account]
#[derive(Default)]
pub struct ConfigProtocol {
    pub split: u64,
}
