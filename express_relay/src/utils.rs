use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, program_error::ProgramError
};

pub fn transfer_lamports(
    from: &AccountInfo,
    to: &AccountInfo,
    amount: u64,
) -> ProgramResult {
    if **from.try_borrow_lamports()? < amount {
        return Err(ProgramError::InsufficientFunds.into());
    }
    **from.try_borrow_mut_lamports()? -= amount;
    **to.try_borrow_mut_lamports()? += amount;
    Ok(())
}
