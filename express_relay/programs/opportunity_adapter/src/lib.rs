pub mod error;
pub mod state;

use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar::instructions as tx_instructions;
use solana_program::{serialize_utils::read_u16, sysvar::instructions::{load_current_index_checked, load_instruction_at_checked}};
use anchor_syn::codegen::program::common::sighash;
use anchor_spl::token::{Mint, Token, TokenAccount};

use crate::{
    error::OpportunityAdapterError,
    state::*,
};

declare_id!("Aoao4VYtcCK94QPD8xEF1ctMHmKuDPEbN11Ls2NphhnV");

#[program]
pub mod opportunity_adapter {
    use super::*;

    pub fn initialize_token_expectations(ctx: Context<InitializeTokenExpectation>, data: InitializeTokenExpectationArgs) -> Result<()> {
        let token_expectation = &mut ctx.accounts.token_expectation;
        let ta_executor = &ctx.accounts.ta_executor;
        let sysvar_ixs = &ctx.accounts.sysvar_instructions;

        let index_permission = load_current_index_checked(sysvar_ixs)?;
        // check that the (index_permission)th instruction from the last matches check
        let num_instructions = read_u16(&mut 0, &sysvar_ixs.data.borrow()).map_err(|_| ProgramError::InvalidInstructionData)?;
        // TODO: do we need to do a checked_sub/saturating_sub here?
        let ix_index = num_instructions - 1 - index_permission;

        let ix = load_instruction_at_checked(ix_index as usize, sysvar_ixs)?;
        let program_equal = ix.program_id == *ctx.program_id;
        let matching_ta_executor = ix.accounts[2].pubkey == ta_executor.key();
        let matching_token_expectation = ix.accounts[3].pubkey == token_expectation.key();
        let matching_accounts = matching_ta_executor && matching_token_expectation;
        let expected_discriminator = sighash("global", "check_token_balance");

        let proper_token_balance_check = program_equal && matching_accounts && ix.data[0..8] == expected_discriminator;

        if !proper_token_balance_check {
            return err!(OpportunityAdapterError::ImproperTokenChecking);
        }

        let expected_change_u64 = data.expected_change.abs() as u64;

        if data.expected_change < 0 {
            token_expectation.balance_post_expected = ta_executor.amount.checked_sub(expected_change_u64).unwrap();
        } else {
            token_expectation.balance_post_expected = ta_executor.amount.checked_add(expected_change_u64).unwrap();
        }

        Ok(())
    }

    pub fn check_token_balance(ctx: Context<CheckTokenBalance>) -> Result<()> {
        let token_expectation = &ctx.accounts.token_expectation;
        let ta_executor = &ctx.accounts.ta_executor;

        if token_expectation.balance_post_expected > ta_executor.amount {
            return err!(OpportunityAdapterError::TokenExpectationNotMet);
        }

        Ok(())
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Eq, PartialEq, Clone, Copy, Debug)]
pub struct InitializeTokenExpectationArgs {
    pub expected_change: i64,
}

#[derive(Accounts)]
pub struct InitializeTokenExpectation<'info> {
    #[account(mut)]
    pub executor: Signer<'info>,
    pub mint: Account<'info, Mint>,
    #[account(token::mint = mint, token::authority = executor)]
    pub ta_executor: Account<'info, TokenAccount>,
    #[account(init, payer = executor, space = RESERVE_TOKEN_EXPECTATION, seeds = [SEED_TOKEN_EXPECTATION, executor.key().as_ref(), mint.key().as_ref()], bump)]
    pub token_expectation: Account<'info, TokenExpectation>,
    // TODO: can we get rid of token_program
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    /// CHECK: this is the sysvar instructions account
    #[account(address = tx_instructions::ID)]
    pub sysvar_instructions: UncheckedAccount<'info>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Eq, PartialEq, Clone, Copy, Debug)]
pub struct CheckTokenBalanceArgs {}

#[derive(Accounts)]
pub struct CheckTokenBalance<'info> {
    #[account(mut)]
    pub executor: Signer<'info>,
    pub mint: Account<'info, Mint>,
    #[account(token::mint = mint, token::authority = executor)]
    pub ta_executor: Account<'info, TokenAccount>,
    #[account(mut, seeds = [SEED_TOKEN_EXPECTATION, executor.key().as_ref(), mint.key().as_ref()], bump, close = executor)]
    pub token_expectation: Account<'info, TokenExpectation>,
    // TODO: can we get rid of token_program
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}
