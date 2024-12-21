use {
    crate::error::ErrorCode,
    anchor_lang::prelude::*,
    anchor_spl::{
        associated_token::get_associated_token_address,
        token_interface::{
            self,
            Mint,
            TokenAccount,
            TokenInterface,
            TransferChecked,
        },
    },
};

pub fn transfer_token_if_needed<'info>(
    from: &InterfaceAccount<'info, TokenAccount>,
    to: &InterfaceAccount<'info, TokenAccount>,
    token_program: &Interface<'info, TokenInterface>,
    authority: &Signer<'info>,
    mint: &InterfaceAccount<'info, Mint>,
    amount: u64,
) -> Result<()> {
    if amount > 0 {
        let cpi_accounts = TransferChecked {
            from:      from.to_account_info(),
            to:        to.to_account_info(),
            mint:      mint.to_account_info(),
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

pub fn check_receiver_token_account<'info>(
    ta: &InterfaceAccount<'info, TokenAccount>,
    mint: &InterfaceAccount<'info, Mint>,
    token_program: &Interface<'info, TokenInterface>,
) -> Result<()> {
    require_eq!(ta.mint, mint.key(), ErrorCode::InvalidMint);
    require_eq!(
        *ta.to_account_info().owner,
        token_program.key(),
        ErrorCode::InvalidTokenProgram
    );

    Ok(())
}

pub fn check_receiver_associated_token_account<'info>(
    ata: &InterfaceAccount<'info, TokenAccount>,
    owner: &Pubkey,
    mint: &InterfaceAccount<'info, Mint>,
    token_program: &Interface<'info, TokenInterface>,
) -> Result<()> {
    require_eq!(
        ata.key(),
        get_associated_token_address(owner, &mint.key()),
        ErrorCode::InvalidAta
    );
    check_receiver_token_account(ata, mint, token_program)?;
    Ok(())
}
