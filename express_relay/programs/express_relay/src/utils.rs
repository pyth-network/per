use anchor_lang::{prelude::*, system_program::{transfer, Transfer}};
use crate::{
    error::ErrorCode,
    state::*,
};

// Validates that the fee split is not larger than the precision
pub fn validate_fee_split(split: u64) -> Result<()> {
    if split > FEE_SPLIT_PRECISION {
        return err!(ErrorCode::FeeSplitLargerThanPrecision);
    }
    Ok(())
}

// Transfers lamports from one program-owned account to another account
pub fn transfer_lamports(
    from: &AccountInfo,
    to: &AccountInfo,
    amount: u64,
) -> Result<()> {
    **from.try_borrow_mut_lamports()? -= amount;
    **to.try_borrow_mut_lamports()? += amount;
    Ok(())
}

// Transfers lamports from one account to another using CPI
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

// Validates that a PDA is correctly derived from the provided program ID and seeds
pub fn validate_pda(pda: &Pubkey, program_id: &Pubkey, seeds: &[&[u8]]) -> Result<()> {
    let (pda_calculated, _) = Pubkey::find_program_address(seeds, program_id);
    if pda != &pda_calculated {
        return err!(ErrorCode::InvalidPDAProvided);
    }

    Ok(())
}
