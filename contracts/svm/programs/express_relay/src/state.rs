use anchor_lang::prelude::*;

pub const FEE_SPLIT_PRECISION: u64 = 10_000;

pub const FEE_SPLIT_PRECISION_PPM: u64 = 1_000_000;
pub const FEE_BPS_TO_PPM: u64 = 100;

pub const RESERVE_EXPRESS_RELAY_METADATA: usize = 8 + 152 + 260;
pub const SEED_METADATA: &[u8] = b"metadata";

#[account]
#[derive(Default)]
pub struct ExpressRelayMetadata {
    pub admin:                    Pubkey,
    pub relayer_signer:           Pubkey,
    pub fee_receiver_relayer:     Pubkey,
    // the portion of the bid that goes to the router, in bps
    pub split_router_default:     u64,
    // the portion of the remaining bid (after router fees) that goes to the relayer, in bps
    pub split_relayer:            u64,
    // the portion of the swap amount that should go to the platform (relayer + express relay), in bps
    pub swap_platform_fee_bps:    u64,
    // secondary relayer signer, useful for 0-downtime transitioning to a new relayer
    pub secondary_relayer_signer: Pubkey,
}

pub const RESERVE_EXPRESS_RELAY_CONFIG_ROUTER: usize = 8 + 40 + 200;
pub const SEED_CONFIG_ROUTER: &[u8] = b"config_router";

#[account]
#[derive(Default)]
pub struct ConfigRouter {
    pub router: Pubkey,
    pub split:  u64,
}
