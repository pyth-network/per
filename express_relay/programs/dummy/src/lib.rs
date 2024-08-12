use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar::instructions as sysvar_instructions;
use express_relay::{self, ID as EXPRESS_RELAY_ID, cpi::accounts::CheckPermission, CheckPermissionArgs};

declare_id!("HYCgALnu6CM2gkQVopa1HGaNf8Vzbs9bomWRiKP267P3");

#[program]
pub mod dummy {
    use super::*;

    pub fn do_nothing(ctx: Context<DoNothing>, data: DoNothingArgs) -> Result<()> {
        assert_eq!(ctx.accounts.protocol.key, ctx.program_id);

        let cpi_program = ctx.accounts.express_relay.to_account_info();
        let cpi_accounts = CheckPermission {
            sysvar_instructions: ctx.accounts.sysvar_instructions.to_account_info(),
            protocol: ctx.accounts.protocol.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
        express_relay::cpi::check_permission(cpi_ctx, CheckPermissionArgs {
            permission_id: data.permission_id,
        })?;

        Ok(())
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Eq, PartialEq, Clone, Copy, Debug)]
pub struct DoNothingArgs {
    pub permission_id: [u8; 32]
}

#[derive(Accounts)]
pub struct DoNothing<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub express_relay: Program<'info, ExpressRelay>,
    /// CHECK: this is the sysvar instructions account
    #[account(address = sysvar_instructions::ID)]
    pub sysvar_instructions: UncheckedAccount<'info>,
    /// CHECK: this is the current program
    pub protocol: UncheckedAccount<'info>,
}

#[account]
#[derive(Default)]
pub struct ExpressRelay;

impl anchor_lang::Id for ExpressRelay {
    fn id() -> Pubkey {
        EXPRESS_RELAY_ID
    }
}
