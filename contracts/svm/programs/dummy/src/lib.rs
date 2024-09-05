use {
    anchor_lang::{
        prelude::*,
        solana_program::sysvar::instructions as sysvar_instructions,
    },
    express_relay::{
        program::ExpressRelay,
        sdk::cpi::check_permission,
    },
};

declare_id!("HYCgALnu6CM2gkQVopa1HGaNf8Vzbs9bomWRiKP267P3");

#[program]
pub mod dummy {
    use super::*;

    pub fn do_nothing(ctx: Context<DoNothing>) -> Result<()> {
        check_permission(
            ctx.accounts.express_relay.key(),
            ctx.accounts.sysvar_instructions.to_account_info(),
            ctx.accounts.permission.to_account_info(),
            ctx.accounts.router.to_account_info(),
        )
    }
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

    /// CHECK: this is the address to receive express relay fees at
    pub router: UncheckedAccount<'info>,
}
