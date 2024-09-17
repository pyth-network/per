use {
    anchor_lang::{
        prelude::*,
        solana_program::sysvar::instructions as sysvar_instructions,
    },
    express_relay::{
        cpi::accounts::CheckPermission,
        program::ExpressRelay,
        sdk::cpi::check_permission_cpi,
        state::{
            ExpressRelayMetadata,
            SEED_CONFIG_ROUTER,
            SEED_METADATA,
        },
    },
};

declare_id!("DUmmYXYFZugRn2DS4REc5F9UbQNoxYsHP1VMZ6j5U7kZ");

#[program]
pub mod dummy {
    use super::*;

    pub fn do_nothing(ctx: Context<DoNothing>) -> Result<()> {
        let check_permission_accounts = CheckPermission {
            sysvar_instructions:    ctx.accounts.sysvar_instructions.to_account_info(),
            permission:             ctx.accounts.permission.to_account_info(),
            router:                 ctx.accounts.router.to_account_info(),
            config_router:          ctx.accounts.config_router.to_account_info(),
            express_relay_metadata: ctx.accounts.express_relay_metadata.to_account_info(),
        };
        let fees = check_permission_cpi(
            check_permission_accounts,
            ctx.accounts.express_relay.to_account_info(),
        )?;
        ctx.accounts.accounting.total_fees += fees;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct DoNothing<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(address = express_relay::ID)]
    pub express_relay: Program<'info, ExpressRelay>,

    #[account(seeds = [SEED_METADATA], bump, seeds::program = express_relay.key())]
    pub express_relay_metadata: Account<'info, ExpressRelayMetadata>,

    /// CHECK: this is the sysvar instructions account
    #[account(address = sysvar_instructions::ID)]
    pub sysvar_instructions: UncheckedAccount<'info>,

    /// CHECK: this is the permission key
    pub permission: UncheckedAccount<'info>,

    /// CHECK: this is the address to receive express relay fees at
    pub router: UncheckedAccount<'info>,

    /// CHECK: doesn't matter what this looks like
    #[account(seeds = [SEED_CONFIG_ROUTER, router.key().as_ref()], bump, seeds::program = express_relay.key())]
    pub config_router: UncheckedAccount<'info>,

    #[account(init_if_needed, payer = payer, space = RESERVE_ACCOUNTING, seeds = [SEED_ACCOUNTING], bump)]
    pub accounting: Account<'info, Accounting>,

    pub system_program: Program<'info, System>,
}

pub const RESERVE_ACCOUNTING: usize = 8 + 8;
pub const SEED_ACCOUNTING: &[u8] = b"accounting";

#[account]
#[derive(Default)]
pub struct Accounting {
    pub total_fees: u64,
}
