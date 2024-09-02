use anchor_syn::codegen::program::common::sighash;
use solana_program::{serialize_utils::read_u16, sysvar::instructions::load_instruction_at_checked};
use anchor_lang::{prelude::*, system_program::{transfer, Transfer}};
use crate::{
    error::ErrorCode,
    state::*,
    SubmitBid,
};

pub fn validate_fee_split(split: u64) -> Result<()> {
    if split > FEE_SPLIT_PRECISION {
        return err!(ErrorCode::FeeSplitLargerThanPrecision);
    }
    Ok(())
}

pub fn transfer_lamports(
    from: &AccountInfo,
    to: &AccountInfo,
    amount: u64,
) -> Result<()> {
    **from.try_borrow_mut_lamports()? -= amount;
    **to.try_borrow_mut_lamports()? += amount;
    Ok(())
}

pub fn transfer_lamports_cpi<'info>(
    from: &AccountInfo<'info>,
    to: &AccountInfo<'info>,
    amount: u64,
    system_program: AccountInfo<'info>,
) -> Result<()> {
    let cpi_accounts = Transfer {
        from: from.clone(),
        to: to.clone(),
    };

    transfer(CpiContext::new(system_program, cpi_accounts), amount)?;

    Ok(())
}

pub fn check_fee_hits_min_rent(account: &AccountInfo, fee: u64) -> Result<()> {
    let balance = account.lamports();
    let rent = Rent::get()?.minimum_balance(account.data_len());
    if balance+fee < rent {
        return err!(ErrorCode::InsufficientRent);
    }

    Ok(())
}

pub struct PermissionInfo {
    pub permission: Pubkey,
    pub router: Pubkey,
}

pub fn num_permissions_in_tx(sysvar_instructions: UncheckedAccount, permission_info: Option<PermissionInfo>) -> Result<u16> {
    let num_instructions = read_u16(&mut 0, &sysvar_instructions.data.borrow()).map_err(|_| ProgramError::InvalidInstructionData)?;
    let mut permission_count = 0u16;
    for index in 0..num_instructions {
        let ix = load_instruction_at_checked(index.into(), &sysvar_instructions)?;

        if ix.program_id != crate::id() {
            continue;
        }
        let expected_discriminator = sighash("global", "submit_bid");
        if ix.data[0..8] != expected_discriminator {
            continue;
        }

        match permission_info {
            Some(ref permission_info) => {
                if ix.accounts[2].pubkey != permission_info.permission {
                    continue;
                }

                if ix.accounts[3].pubkey != permission_info.router {
                    continue;
                }
            }
            None => {}
        }

        permission_count += 1;
    }

    Ok(permission_count)
}

pub fn handle_bid_payment(ctx: Context<SubmitBid>, bid_amount: u64) -> Result<()> {
    let searcher = &ctx.accounts.searcher;
    let rent_searcher = Rent::get()?.minimum_balance(searcher.to_account_info().data_len());
    if bid_amount + rent_searcher > searcher.lamports() {
        return err!(ErrorCode::InsufficientSearcherFunds);
    }

    let express_relay_metadata = &ctx.accounts.express_relay_metadata;
    let split_relayer = express_relay_metadata.split_relayer;
    let split_router_default = express_relay_metadata.split_router_default;

    let split_router: u64;
    let router_config = &ctx.accounts.router_config;
    let router_config_account_info = router_config.to_account_info();
    // validate the router config account struct in program logic bc it may be uninitialized
    // only validate if the account has data
    if router_config_account_info.data_len() > 0 {
        let account_data = &mut &**router_config_account_info.try_borrow_data()?;
        let router_config_data = ConfigRouter::try_deserialize(account_data)?;
        split_router = router_config_data.split;
    } else {
        split_router = split_router_default;
    }

    let fee_router = bid_amount * split_router / FEE_SPLIT_PRECISION;
    if fee_router > bid_amount {
        // this error should never be reached due to fee split checks, but kept as a matter of defensive programming
        return err!(ErrorCode::FeesHigherThanBid);
    }
    if fee_router > 0 {
        check_fee_hits_min_rent(&ctx.accounts.router, fee_router)?;

        transfer_lamports_cpi(
            &searcher.to_account_info(),
            &ctx.accounts.router.to_account_info(),
            fee_router,
            ctx.accounts.system_program.to_account_info()
        )?;
    }

    let fee_relayer = bid_amount.saturating_sub(fee_router) * split_relayer / FEE_SPLIT_PRECISION;
    if fee_relayer.checked_add(fee_router).ok_or(ProgramError::ArithmeticOverflow)? > bid_amount {
        // this error should never be reached due to fee split checks, but kept as a matter of defensive programming
        return err!(ErrorCode::FeesHigherThanBid);
    }
    if fee_relayer > 0 {
        check_fee_hits_min_rent(&ctx.accounts.fee_receiver_relayer, fee_relayer)?;

        transfer_lamports_cpi(
            &searcher.to_account_info(),
            &ctx.accounts.fee_receiver_relayer.to_account_info(),
            fee_relayer,
            ctx.accounts.system_program.to_account_info()
        )?;
    }

    transfer_lamports_cpi(
        &searcher.to_account_info(),
        &express_relay_metadata.to_account_info(),
        bid_amount.saturating_sub(fee_router).saturating_sub(fee_relayer),
        ctx.accounts.system_program.to_account_info()
    )?;

    Ok(())
}
