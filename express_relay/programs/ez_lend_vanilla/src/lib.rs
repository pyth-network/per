use anchor_lang::{prelude::*, system_program::System};
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer as SplTransfer};

declare_id!("AwBnwAgYZoGjZ1L1ADQ2uYBLg5uTUPwHKYCFt9peGWWC");

#[program]
pub mod ez_lend_vanilla {
    use super::*;

    pub fn create_token_acc(_ctx: Context<CreateTokenAcc>, _data: CreateTokenAccArgs) -> Result<()> {
        Ok(())
    }

    pub fn create_vault(ctx: Context<CreateVault>, data: CreateVaultArgs) -> Result<()> {
        let vault = &mut ctx.accounts.vault;
        let payer = &ctx.accounts.payer;
        let collateral_ata_payer = &ctx.accounts.collateral_ata_payer;
        let collateral_ta_program = &ctx.accounts.collateral_ta_program;
        let debt_ata_payer = &ctx.accounts.debt_ata_payer;
        let debt_ta_program = &ctx.accounts.debt_ta_program;
        let debt_mint = &ctx.accounts.debt_mint;
        let token_program = &ctx.accounts.token_program;

        // transfer collateral from payer to vault
        let cpi_accounts = SplTransfer {
            from: collateral_ata_payer.to_account_info().clone(),
            to: collateral_ta_program.to_account_info().clone(),
            authority: payer.to_account_info().clone(),
        };
        let cpi_program = token_program.to_account_info();
        token::transfer(
            CpiContext::new(cpi_program, cpi_accounts),
            data.collateral_amount)?;


        // transfer debt from vault to payer
        let cpi_accounts = SplTransfer {
            from: debt_ta_program.to_account_info().clone(),
            to: debt_ata_payer.to_account_info().clone(),
            authority: debt_ta_program.to_account_info().clone(),
        };
        let cpi_program = token_program.to_account_info();
        let debt_mint_pk = debt_mint.key();
        let cpi_seeds = &[
            "ata".as_bytes(),
            debt_mint_pk.as_ref(),
            &[ctx.bumps.debt_ta_program]
        ];
        let signer_seeds = &[&cpi_seeds[..]];
        token::transfer(
            CpiContext::new_with_signer(cpi_program, cpi_accounts, signer_seeds),
            data.debt_amount)?;


        vault.bump = ctx.bumps.vault;
        vault.collateral_mint = ctx.accounts.collateral_mint.key();
        vault.collateral_amount = data.collateral_amount;
        vault.debt_mint = ctx.accounts.debt_mint.key();
        vault.debt_amount = data.debt_amount;

        Ok(())
    }

    pub fn liquidate(ctx: Context<Liquidate>, _data: LiquidateArgs) -> Result<()> {
        let vault = &mut ctx.accounts.vault;
        let payer = &ctx.accounts.payer;
        let collateral_ata_payer = &ctx.accounts.collateral_ata_payer;
        let collateral_ta_program = &ctx.accounts.collateral_ta_program;
        let debt_ata_payer = &ctx.accounts.debt_ata_payer;
        let debt_ta_program = &ctx.accounts.debt_ta_program;
        let collateral_mint = &ctx.accounts.collateral_mint;
        let token_program = &ctx.accounts.token_program;

        // transfer debt from payer to vault
        let cpi_accounts = SplTransfer {
            from: debt_ata_payer.to_account_info().clone(),
            to: debt_ta_program.to_account_info().clone(),
            authority: payer.to_account_info().clone(),
        };
        let cpi_program = token_program.to_account_info();

        token::transfer(
            CpiContext::new(cpi_program, cpi_accounts),
            vault.debt_amount)?;

        // transfer collateral from vault to payer
        let cpi_accounts = SplTransfer {
            from: collateral_ta_program.to_account_info().clone(),
            to: collateral_ata_payer.to_account_info().clone(),
            authority: collateral_ta_program.to_account_info().clone(),
        };
        let cpi_program = token_program.to_account_info();
        let collateral_mint_pk = collateral_mint.key();
        let cpi_seeds = &[
            "ata".as_bytes(),
            collateral_mint_pk.as_ref(),
            &[ctx.bumps.collateral_ta_program]
        ];
        let signer_seeds = &[&cpi_seeds[..]];
        token::transfer(
            CpiContext::new_with_signer(cpi_program, cpi_accounts, signer_seeds),
            vault.collateral_amount)?;

        vault.collateral_amount = 0;
        vault.debt_amount = 0;

        Ok(())
    }
}

#[account]
#[derive(Default)]
pub struct Vault {
    pub bump: u8,
    pub collateral_mint: Pubkey,
    pub collateral_amount: u64,
    pub debt_mint: Pubkey,
    pub debt_amount: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Eq, PartialEq, Clone, Debug)]
pub struct CreateTokenAccArgs {}

#[derive(Accounts)]
pub struct CreateTokenAcc<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub mint: Account<'info, Mint>,
    #[account(
        init,
        payer = payer,
        seeds = [b"ata", mint.key().as_ref()],
        bump,
        token::mint = mint,
        token::authority = token_account
    )]
    pub token_account: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Eq, PartialEq, Clone, Debug)]
pub struct CreateVaultArgs {
    pub vault_id: [u8; 32],
    pub collateral_amount: u64,
    pub debt_amount: u64,
}

#[derive(Accounts)]
#[instruction(data: CreateVaultArgs)]
pub struct CreateVault<'info> {
    #[account(init, payer = payer, space = 8 + 1 + 32 + 8 + 32 + 8, seeds = [b"vault".as_ref(), &data.vault_id], bump)]
    pub vault: Account<'info, Vault>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub collateral_mint: Account<'info, Mint>,
    pub debt_mint: Account<'info, Mint>,
    #[account(mut, token::mint = collateral_mint, token::authority = payer)]
    pub collateral_ata_payer: Account<'info, TokenAccount>,
    #[account(
        init_if_needed,
        payer = payer,
        seeds = [b"ata", collateral_mint.key().as_ref()],
        bump,
        token::mint = collateral_mint,
        token::authority = collateral_ta_program
    )]
    pub collateral_ta_program: Account<'info, TokenAccount>,
    #[account(mut, token::mint = debt_mint, token::authority = payer)]
    pub debt_ata_payer: Account<'info, TokenAccount>,
    #[account(
        init_if_needed,
        payer = payer,
        seeds = [b"ata", debt_mint.key().as_ref()],
        bump,
        token::mint = debt_mint,
        token::authority = debt_ta_program
    )]
    pub debt_ta_program: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Eq, PartialEq, Clone, Copy, Debug)]
pub struct LiquidateArgs {
    pub vault_id: [u8; 32],
}

#[derive(Accounts)]
#[instruction(data: LiquidateArgs)]
pub struct Liquidate<'info> {
    #[account(mut, seeds = [b"vault".as_ref(), &data.vault_id], bump = vault.bump)]
    pub vault: Account<'info, Vault>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub collateral_mint: Account<'info, Mint>,
    pub debt_mint: Account<'info, Mint>,
    #[account(mut, token::mint = collateral_mint, token::authority = payer)]
    pub collateral_ata_payer: Account<'info, TokenAccount>,
    #[account(
        mut,
        seeds = [b"ata", collateral_mint.key().as_ref()],
        bump,
        token::mint = collateral_mint
    )]
    pub collateral_ta_program: Account<'info, TokenAccount>,
    #[account(mut, token::mint = debt_mint, token::authority = payer)]
    pub debt_ata_payer: Account<'info, TokenAccount>,
    #[account(
        mut,
        seeds = [b"ata", debt_mint.key().as_ref()],
        bump,
        token::mint = debt_mint
    )]
    pub debt_ta_program: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}
