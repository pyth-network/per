use anchor_lang::prelude::*;

pub const FEE_SPLIT_PRECISION: u64 = 10_000;

pub const SEED_EXPRESS_RELAY_FEES: &[u8] = b"express_relay_fees";

pub const RESERVE_EXPRESS_RELAY_METADATA: usize = 8+112;
pub const SEED_METADATA: &[u8] = b"metadata";

#[account]
#[derive(Default)]
pub struct ExpressRelayMetadata {
    pub admin: Pubkey,
    pub relayer_signer: Pubkey,
    pub fee_receiver_relayer: Pubkey,
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
