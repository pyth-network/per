use {
    anchor_lang::{
        prelude::*,
        solana_program::sysvar::instructions as sysvar_instructions,
    },
    express_relay::{
        self,
        cpi::accounts::CheckPermission,
        program::ExpressRelay,
    },
};

declare_id!("HYCgALnu6CM2gkQVopa1HGaNf8Vzbs9bomWRiKP267P3");

#[program]
pub mod dummy {
    use super::*;

    pub fn do_nothing(ctx: Context<DoNothing>) -> Result<()> {
        // just want to check if the permission is valid, and do nothing else
        let cpi_program = ctx.accounts.express_relay.to_account_info();
        let cpi_accounts = CheckPermission {
            sysvar_instructions: ctx.accounts.sysvar_instructions.to_account_info(),
            permission:          ctx.accounts.permission.to_account_info(),
            router:              ctx.accounts.router.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
        express_relay::cpi::check_permission(cpi_ctx)?;

        Ok(())
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
