use {
    anchor_lang::{
        prelude::*,
        solana_program::sysvar::instructions as sysvar_instructions,
    },
    express_relay::{
        program::ExpressRelay,
        sdk::{
            cpi::check_permission,
            fees::get_fees_paid_to_router,
        },
        state::{
            ExpressRelayMetadata,
            SEED_CONFIG_ROUTER,
        },
    },
};

declare_id!("HYCgALnu6CM2gkQVopa1HGaNf8Vzbs9bomWRiKP267P3");

#[program]
pub mod dummy {
    use super::*;

    pub fn do_nothing(ctx: Context<DoNothing>) -> Result<()> {
        check_permission(
            ctx.accounts.sysvar_instructions.to_account_info(),
            ctx.accounts.permission.to_account_info(),
            ctx.accounts.router.to_account_info(),
        )
    }

    pub fn count_fees(ctx: Context<CountFees>) -> Result<()> {
        let fees_paid = get_fees_paid_to_router(
            ctx.accounts.sysvar_instructions.to_account_info(),
            ctx.accounts.permission.to_account_info(),
            ctx.accounts.router.to_account_info(),
            ctx.accounts.router_config.to_account_info(),
            ctx.accounts.express_relay_metadata.clone(),
        )?;

        ctx.accounts.fees_count.count += fees_paid;

        Ok(())
    }
}

#[derive(Accounts)]
pub struct DoNothing<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(address = express_relay::ID)]
    pub express_relay: Program<'info, ExpressRelay>,

    /// CHECK: this is the sysvar instructions account
    #[account(address = sysvar_instructions::ID)]
    pub sysvar_instructions: UncheckedAccount<'info>,

    /// CHECK: this is the permission key
    pub permission: UncheckedAccount<'info>,

    /// CHECK: this is the address to receive express relay fees at
    pub router: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct CountFees<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(seeds = [b"metadata"], bump, seeds::program = express_relay::ID)]
    pub express_relay_metadata: Account<'info, ExpressRelayMetadata>,

    /// CHECK: this is the sysvar instructions account
    #[account(address = sysvar_instructions::ID)]
    pub sysvar_instructions: UncheckedAccount<'info>,

    /// CHECK: this is the permission key
    pub permission: UncheckedAccount<'info>,

    /// CHECK: this is the address to receive express relay fees at
    pub router: UncheckedAccount<'info>,

    /// CHECK: doesn't matter what this looks like
    #[account(seeds = [SEED_CONFIG_ROUTER, router.key().as_ref()], bump, seeds::program = express_relay::ID)]
    pub router_config: UncheckedAccount<'info>,

    #[account(init_if_needed, payer = payer, space = 8 + 8, seeds = [SEED_FEES_COUNT], bump)]
    pub fees_count: Account<'info, FeesCount>,

    pub system_program: Program<'info, System>,
}

pub const SEED_FEES_COUNT: &[u8] = b"fees_count";

#[account]
#[derive(Default)]
pub struct FeesCount {
    pub count: u64,
}
