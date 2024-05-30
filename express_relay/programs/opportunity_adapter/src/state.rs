use anchor_lang::prelude::*;

pub const RESERVE_TOKEN_EXPECTATION: usize = 8+8;
pub const SEED_TOKEN_EXPECTATION: &[u8] = b"token_expectation";

#[account]
#[derive(Default)]
pub struct TokenExpectation {
    pub balance_post_expected: u64
}

pub const RESERVE_AUTHORITY: usize = 8+0;
pub const SEED_AUTHORITY: &[u8] = b"authority";

#[account]
#[derive(Default)]
pub struct Authority {}

pub struct TokenAmount {
    pub mint: Pubkey,
    pub amount: u64,
}

pub const RESERVE_SIGNATURE_ACCOUNTING: usize = 8+0;
pub const SEED_SIGNATURE_ACCOUNTING: &[u8] = b"signature_accounting";

#[account]
#[derive(Default)]
pub struct SignatureAccounting {}
