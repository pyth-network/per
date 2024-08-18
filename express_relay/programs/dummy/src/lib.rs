use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar::instructions as sysvar_instructions;
use express_relay::{self, program::ExpressRelay, cpi::accounts::CheckPermission, state::SEED_EXPRESS_RELAY_FEES};

declare_id!("HYCgALnu6CM2gkQVopa1HGaNf8Vzbs9bomWRiKP267P3");

#[program]
pub mod dummy {
    use super::*;

    pub fn initialize(_ctx: Context<Initialize>) -> Result<()> {
        Ok(())
    }

    pub fn do_nothing(ctx: Context<DoNothing>) -> Result<()> {
        let cpi_program = ctx.accounts.express_relay.to_account_info();
        let cpi_accounts = CheckPermission {
            sysvar_instructions: ctx.accounts.sysvar_instructions.to_account_info(),
            permission: ctx.accounts.permission.to_account_info(),
            protocol: ctx.accounts.protocol.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
        express_relay::cpi::check_permission(cpi_ctx)?;

        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    /// CHECK: don't care what this PDA looks like
    #[account(init, payer = payer, space = 0, seeds = [SEED_EXPRESS_RELAY_FEES], bump)]
    pub fee_receiver_express_relay: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct DoNothing<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    pub express_relay: Program<'info, ExpressRelay>,

    /// CHECK: this is the sysvar instructions account
    #[account(address = sysvar_instructions::ID)]
    pub sysvar_instructions: UncheckedAccount<'info>,

    /// CHECK: this is the permission key
    pub permission: UncheckedAccount<'info>,

    /// CHECK: this is the current program
    #[account(address = crate::ID)]
    pub protocol: UncheckedAccount<'info>,
}
