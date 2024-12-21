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

pub fn check_ata(ata: &Pubkey, owner: &Pubkey, mint: &Pubkey) -> Result<()> {
    require!(
        *ata == get_associated_token_address(owner, mint),
        ErrorCode::InvalidAta
    );
    Ok(())
}
