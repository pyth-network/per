use anchor_lang::prelude::*;

pub const FEE_SPLIT_PRECISION: u64 = 10_000;

pub const SEED_EXPRESS_RELAY_FEES: &[u8] = b"express_relay_fees";

pub const RESERVE_PERMISSION: usize = 8+16;
pub const SEED_PERMISSION: &[u8] = b"permission";

#[account(zero_copy)]
#[derive(Default)]
pub struct PermissionMetadata {
    pub balance: u64,
    pub bid_amount: u64,
}

pub const RESERVE_EXPRESS_RELAY_METADATA: usize = 8+112;
pub const SEED_METADATA: &[u8] = b"metadata";

#[account(zero_copy)]
#[derive(Default)]
pub struct ExpressRelayMetadata {
    pub admin: Pubkey,
    pub relayer_signer: Pubkey,
    pub relayer_fee_receiver: Pubkey,
    pub split_protocol_default: u64,
    pub split_relayer: u64,
}

pub const RESERVE_EXPRESS_RELAY_CONFIG_PROTOCOL: usize = 8+8;
pub const SEED_CONFIG_PROTOCOL: &[u8] = b"config_protocol";

#[account]
#[derive(Default)]
pub struct ConfigProtocol {
    pub split: u64,
}

pub const RESERVE_AUTHORITY: usize = 8+0;
pub const SEED_AUTHORITY: &[u8] = b"authority";

#[account(zero_copy)]
#[derive(Default)]
pub struct Authority {}

pub const RESERVE_SIGNATURE_ACCOUNTING: usize = 8+0;
pub const SEED_SIGNATURE_ACCOUNTING: &[u8] = b"signature_accounting";

#[account(zero_copy)]
#[derive(Default)]
pub struct SignatureAccounting {}
