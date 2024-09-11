use {
    crate::{
        error::ErrorCode,
        state::*,
        SubmitBid,
        SubmitBidArgs,
    },
    anchor_lang::{
        prelude::*,
        solana_program::{
            instruction::Instruction,
            serialize_utils::read_u16,
            sysvar::instructions::load_instruction_at_checked,
        },
        system_program::{
            transfer,
            Transfer,
        },
        Discriminator,
    },
};

pub fn validate_fee_split(split: u64) -> Result<()> {
    if split > FEE_SPLIT_PRECISION {
        return err!(ErrorCode::FeeSplitLargerThanPrecision);
    }
    Ok(())
}

pub fn transfer_lamports(from: &AccountInfo, to: &AccountInfo, amount: u64) -> Result<()> {
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
        to:   to.clone(),
    };

    transfer(CpiContext::new(system_program, cpi_accounts), amount)?;

    Ok(())
}

pub fn check_fee_hits_min_rent(account: &AccountInfo, fee: u64) -> Result<()> {
    let balance = account.lamports();
    let rent = Rent::get()?.minimum_balance(account.data_len());
    if balance + fee < rent {
        return err!(ErrorCode::InsufficientRent);
    }

    Ok(())
}

pub struct PermissionInfo<'info> {
    pub permission:             Pubkey,
    pub router:                 Pubkey,
    pub config_router:          AccountInfo<'info>,
    pub express_relay_metadata: AccountInfo<'info>,
}

pub fn get_matching_instructions(
    sysvar_instructions: AccountInfo,
    permission_info: Option<&PermissionInfo>,
) -> Result<Vec<Instruction>> {
    let num_instructions = read_u16(&mut 0, &sysvar_instructions.data.borrow())
        .map_err(|_| ProgramError::InvalidInstructionData)?;
    let mut matching_instructions = Vec::new();
    for index in 0..num_instructions {
        let ix = load_instruction_at_checked(index.into(), &sysvar_instructions)?;

        if ix.program_id != crate::id() {
            continue;
        }
        if ix.data[0..8] != crate::instruction::SubmitBid::DISCRIMINATOR {
            continue;
        }

        if let Some(permission_info) = permission_info {
            if ix.accounts[2].pubkey != permission_info.permission {
                continue;
            }

            if ix.accounts[3].pubkey != permission_info.router {
                continue;
            }

            if ix.accounts[4].pubkey != permission_info.config_router.key() {
                continue;
            }

            if ix.accounts[5].pubkey != permission_info.express_relay_metadata.key() {
                continue;
            }
        }

        matching_instructions.push(ix);
    }

    Ok(matching_instructions)
}

/// Extracts the bid paid from a SubmitBid instruction
pub fn extract_bid_from_submit_bid_ix(submit_bid_ix: &Instruction) -> Result<u64> {
    let submit_bid_args =
        SubmitBidArgs::try_from_slice(&submit_bid_ix.data[8..]).map_err(|_| {
            ProgramError::BorshIoError("Failed to deserialize SubmitBidArgs".to_string())
        })?;
    Ok(submit_bid_args.bid_amount)
}

/// Performs instruction introspection on a transaction to determine the SubmitBid instructions that match the given permission and router
/// Returns the number of matching instructions. If the permission_info struct is provided, also returns the total fees paid to the router in the matching instructions
pub fn inspect_permissions_in_tx(
    sysvar_instructions: UncheckedAccount,
    permission_info: Option<&PermissionInfo>,
) -> Result<(u16, Option<u64>)> {
    let matching_ixs =
        get_matching_instructions(sysvar_instructions.to_account_info(), permission_info)?;
    let n_ixs = matching_ixs.len() as u16;

    match permission_info {
        Some(permission_info) => {
            let mut total_fees = 0u64;
            let data_config_router = &mut &**permission_info.config_router.try_borrow_data()?;
            let split_router = match ConfigRouter::try_deserialize(data_config_router) {
                Ok(config_router) => config_router.split,
                Err(_) => {
                    let data_express_relay_metadata =
                        &mut &**permission_info.express_relay_metadata.try_borrow_data()?;
                    let express_relay_metadata =
                        ExpressRelayMetadata::try_deserialize(data_express_relay_metadata)
                            .map_err(|_| ProgramError::InvalidAccountData)?;
                    express_relay_metadata.split_router_default
                }
            };
            for ix in matching_ixs {
                let fee = extract_bid_from_submit_bid_ix(&ix)?;
                total_fees += fee * split_router / FEE_SPLIT_PRECISION;
            }

            Ok((n_ixs, Some(total_fees)))
        }
        None => Ok((n_ixs, None)),
    }
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

    let config_router = &ctx.accounts.config_router;
    let config_router_account_info = config_router.to_account_info();
    // validate the router config account struct in program logic bc it may be uninitialized
    // only validate if the account has data
    let split_router: u64 = if config_router_account_info.data_len() > 0 {
        let account_data = &mut &**config_router_account_info.try_borrow_data()?;
        let config_router_data = ConfigRouter::try_deserialize(account_data)?;
        config_router_data.split
    } else {
        split_router_default
    };

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
            ctx.accounts.system_program.to_account_info(),
        )?;
    }

    let fee_relayer = bid_amount.saturating_sub(fee_router) * split_relayer / FEE_SPLIT_PRECISION;
    if fee_relayer
        .checked_add(fee_router)
        .ok_or(ProgramError::ArithmeticOverflow)?
        > bid_amount
    {
        // this error should never be reached due to fee split checks, but kept as a matter of defensive programming
        return err!(ErrorCode::FeesHigherThanBid);
    }
    if fee_relayer > 0 {
        check_fee_hits_min_rent(&ctx.accounts.fee_receiver_relayer, fee_relayer)?;

        transfer_lamports_cpi(
            &searcher.to_account_info(),
            &ctx.accounts.fee_receiver_relayer.to_account_info(),
            fee_relayer,
            ctx.accounts.system_program.to_account_info(),
        )?;
    }

    transfer_lamports_cpi(
        &searcher.to_account_info(),
        &express_relay_metadata.to_account_info(),
        bid_amount
            .saturating_sub(fee_router)
            .saturating_sub(fee_relayer),
        ctx.accounts.system_program.to_account_info(),
    )?;

    Ok(())
}
