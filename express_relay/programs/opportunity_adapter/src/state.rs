use anchor_lang::prelude::*;

pub const RESERVE_TOKEN_EXPECTATION: usize = 200;
pub const SEED_TOKEN_EXPECTATION: &[u8] = b"token_expectation";

#[account]
#[derive(Default)]
pub struct TokenExpectation {
    pub balance_post_expected: u64
}
