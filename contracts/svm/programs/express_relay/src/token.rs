use {
    anchor_lang::prelude::*,
    anchor_spl::token_interface::{
        self,
        Mint,
        TokenAccount,
        TokenInterface,
        TransferChecked,
    },
};

pub fn get_token_account_checked(
    account: &AccountInfo,
    expected_program_id: &Pubkey,
) -> Result<TokenAccount> {
    if account.data_len() == 0 {
        return Err(ErrorCode::AccountNotInitialized.into());
    }

    if account.owner != expected_program_id {
        return Err(ErrorCode::ConstraintTokenTokenProgram.into());
    }

    let token_account = match TokenAccount::try_deserialize(&mut &account.data.borrow()[..]) {
        Ok(ta) => ta,
        Err(_) => return Err(ErrorCode::AccountDidNotDeserialize.into()),
    };

    Ok(token_account)
}
pub fn transfer_token_if_needed<'info>(
    from: &InterfaceAccount<'info, TokenAccount>,
    to: AccountInfo<'info>,
    token_program: &Interface<'info, TokenInterface>,
    authority: &Signer<'info>,
    mint: &InterfaceAccount<'info, Mint>,
    amount: u64,
) -> Result<()> {
    if amount > 0 {
        let cpi_accounts = TransferChecked {
            from: from.to_account_info(),
            to,
            mint: mint.to_account_info(),
            authority: authority.to_account_info(),
        };

        token_interface::transfer_checked(
            CpiContext::new(token_program.to_account_info(), cpi_accounts),
            amount,
            mint.decimals,
        )?;
    }
    Ok(())
}

pub fn check_receiver_and_transfer_token_if_needed<'info>(
    from: &InterfaceAccount<'info, TokenAccount>,
    recipient_token_account: &UncheckedAccount<'info>,
    recipient: Option<&Pubkey>,
    token_program: &Interface<'info, TokenInterface>,
    authority: &Signer<'info>,
    mint: &InterfaceAccount<'info, Mint>,
    amount: u64,
) -> Result<()> {
    if amount > 0 {
        let to = get_token_account_checked(
            &recipient_token_account.to_account_info(),
            &token_program.key(),
        )?;

        if let Some(recipient) = recipient {
            if !to.owner.eq(recipient) {
                return Err(ErrorCode::ConstraintTokenOwner.into());
            }
        }
        if !to.mint.eq(&mint.key()) {
            return Err(ErrorCode::ConstraintTokenMint.into());
        }
        transfer_token_if_needed(
            from,
            recipient_token_account.to_account_info(),
            token_program,
            authority,
            mint,
            amount,
        )?;
    }
    Ok(())
}
