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

pub fn num_permissions_in_tx(sysvar_instructions: UncheckedAccount, permission: Option<Pubkey>, router: Option<Pubkey>) -> Result<u16> {
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

        match permission {
            Some(permission) => {
                if ix.accounts[2].pubkey != permission {
                    continue;
                }
            }
            None => {}
        }

        match router {
            Some(router) => {
                if ix.accounts[3].pubkey != router {
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

    let fee_relayer = bid_amount.saturating_sub(fee_router) * split_relayer / FEE_SPLIT_PRECISION;
    if fee_relayer.checked_add(fee_router).ok_or(ProgramError::ArithmeticOverflow)? > bid_amount {
        // this error should never be reached due to fee split checks, but kept as a matter of defensive programming
        return err!(ErrorCode::FeesHigherThanBid);
    }

    let router = &ctx.accounts.router;
    let balance_router = router.lamports();
    let rent_router = Rent::get()?.minimum_balance(router.to_account_info().data_len());
    if balance_router+fee_router < rent_router {
        return err!(ErrorCode::InsufficientRouterRent);
    }

    let fee_receiver_relayer = &ctx.accounts.fee_receiver_relayer;
    let balance_fee_receiver_relayer = fee_receiver_relayer.lamports();
    let rent_fee_receiver_relayer = Rent::get()?.minimum_balance(fee_receiver_relayer.to_account_info().data_len());
    if balance_fee_receiver_relayer+fee_relayer < rent_fee_receiver_relayer {
        return err!(ErrorCode::InsufficientRelayerFeeReceiverRent);
    }

    transfer_lamports_cpi(
        &searcher.to_account_info(),
        &router.to_account_info(),
        fee_router,
        ctx.accounts.system_program.to_account_info()
    )?;
    transfer_lamports_cpi(
        &searcher.to_account_info(),
        &fee_receiver_relayer.to_account_info(),
        fee_relayer,
        ctx.accounts.system_program.to_account_info()
    )?;
    transfer_lamports_cpi(
        &searcher.to_account_info(),
        &express_relay_metadata.to_account_info(),
        bid_amount.saturating_sub(fee_router).saturating_sub(fee_relayer),
        ctx.accounts.system_program.to_account_info()
    )?;

    Ok(())
}
