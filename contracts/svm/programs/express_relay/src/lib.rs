#![allow(unexpected_cfgs)] // https://github.com/coral-xyz/anchor/issues/3401

pub mod clock;
pub mod error;
pub mod sdk;
pub mod state;
pub mod swap;
pub mod token;
pub mod utils;

use {
    crate::{
        clock::check_deadline,
        error::ErrorCode,
        state::*,
        swap::PostFeeSwapArgs,
        token::transfer_token_if_needed,
        utils::*,
    },
    anchor_lang::{
        prelude::*,
        solana_program::{
            instruction::{
                get_stack_height,
                TRANSACTION_LEVEL_STACK_HEIGHT,
            },
            sysvar::instructions as sysvar_instructions,
        },
        system_program::System,
    },
    anchor_spl::token_interface::{
        Mint,
        TokenAccount,
        TokenInterface,
    },
};

declare_id!("PytERJFhAKuNNuaiXkApLfWzwNwSNDACpigT3LwQfou");

#[program]
pub mod express_relay {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>, data: InitializeArgs) -> Result<()> {
        validate_fee_split(data.split_router_default)?;
        validate_fee_split(data.split_relayer)?;

        let express_relay_metadata_data = &mut ctx.accounts.express_relay_metadata;

        express_relay_metadata_data.admin = *ctx.accounts.admin.key;
        express_relay_metadata_data.relayer_signer = *ctx.accounts.relayer_signer.key;
        express_relay_metadata_data.fee_receiver_relayer = *ctx.accounts.fee_receiver_relayer.key;
        express_relay_metadata_data.split_router_default = data.split_router_default;
        express_relay_metadata_data.split_relayer = data.split_relayer;

        Ok(())
    }

    pub fn set_admin(ctx: Context<SetAdmin>) -> Result<()> {
        let express_relay_metadata_data = &mut ctx.accounts.express_relay_metadata;

        express_relay_metadata_data.admin = *ctx.accounts.admin_new.key;

        Ok(())
    }

    pub fn set_relayer(ctx: Context<SetRelayer>) -> Result<()> {
        let express_relay_metadata_data = &mut ctx.accounts.express_relay_metadata;

        express_relay_metadata_data.relayer_signer = *ctx.accounts.relayer_signer.key;
        express_relay_metadata_data.fee_receiver_relayer = *ctx.accounts.fee_receiver_relayer.key;

        Ok(())
    }

    pub fn set_secondary_relayer(ctx: Context<SetSecondaryRelayer>) -> Result<()> {
        let express_relay_metadata_data = &mut ctx.accounts.express_relay_metadata;

        express_relay_metadata_data.secondary_relayer_signer =
            *ctx.accounts.secondary_relayer_signer.key;

        Ok(())
    }

    pub fn set_splits(ctx: Context<SetSplits>, data: SetSplitsArgs) -> Result<()> {
        validate_fee_split(data.split_router_default)?;
        validate_fee_split(data.split_relayer)?;

        let express_relay_metadata_data = &mut ctx.accounts.express_relay_metadata;

        express_relay_metadata_data.split_router_default = data.split_router_default;
        express_relay_metadata_data.split_relayer = data.split_relayer;

        Ok(())
    }

    pub fn set_swap_platform_fee(
        ctx: Context<SetSplits>,
        data: SetSwapPlatformFeeArgs,
    ) -> Result<()> {
        validate_fee_split(data.swap_platform_fee_bps)?;

        ctx.accounts.express_relay_metadata.swap_platform_fee_bps = data.swap_platform_fee_bps;

        Ok(())
    }

    pub fn set_router_split(ctx: Context<SetRouterSplit>, data: SetRouterSplitArgs) -> Result<()> {
        validate_fee_split(data.split_router)?;

        ctx.accounts.config_router.router = *ctx.accounts.router.key;
        ctx.accounts.config_router.split = data.split_router;

        Ok(())
    }

    /// Submits a bid for a particular (permission, router) pair and distributes bids according to splits.
    pub fn submit_bid(ctx: Context<SubmitBid>, data: SubmitBidArgs) -> Result<()> {
        check_deadline(data.deadline)?;
        ctx.accounts
            .express_relay_metadata
            .check_relayer_signer(ctx.accounts.relayer_signer.key)?;

        // check that not cpi.
        if get_stack_height() > TRANSACTION_LEVEL_STACK_HEIGHT {
            return err!(ErrorCode::InvalidCPISubmitBid);
        }

        // Check "no reentrancy"--SubmitBid instruction only used once in transaction.
        // This is done to prevent an exploit where a searcher submits a transaction with multiple `SubmitBid`` instructions with different permission keys.
        // That could allow the searcher to win the right to perform the transaction if they won just one of the auctions.
        let matching_ixs = get_matching_submit_bid_instructions(
            ctx.accounts.sysvar_instructions.to_account_info(),
            None,
        )?;
        if matching_ixs.len() > 1 {
            return err!(ErrorCode::MultiplePermissions);
        }

        handle_bid_payment(ctx, data.bid_amount)
    }

    /// Checks if permissioning exists for a particular (permission, router) pair within the same transaction.
    /// Permissioning takes the form of a SubmitBid instruction with matching permission and router accounts.
    /// Returns the fees paid to the router in the matching instructions.
    pub fn check_permission(ctx: Context<CheckPermission>) -> Result<u64> {
        let (num_permissions, total_router_fees) = inspect_permissions_in_tx(
            ctx.accounts.sysvar_instructions.clone(),
            PermissionInfo {
                permission:             *ctx.accounts.permission.key,
                router:                 *ctx.accounts.router.key,
                config_router:          ctx.accounts.config_router.to_account_info(),
                express_relay_metadata: ctx.accounts.express_relay_metadata.to_account_info(),
            },
        )?;

        if num_permissions == 0 {
            return err!(ErrorCode::MissingPermission);
        }

        Ok(total_router_fees)
    }

    pub fn withdraw_fees(ctx: Context<WithdrawFees>) -> Result<()> {
        let express_relay_metadata = &ctx.accounts.express_relay_metadata;
        let fee_receiver_admin = &ctx.accounts.fee_receiver_admin;

        let express_relay_metadata_account_info = express_relay_metadata.to_account_info();
        let rent_express_relay_metadata =
            Rent::get()?.minimum_balance(express_relay_metadata_account_info.data_len());

        let amount = express_relay_metadata_account_info
            .lamports()
            .saturating_sub(rent_express_relay_metadata);
        if amount == 0 {
            return Ok(());
        }
        transfer_lamports(
            &express_relay_metadata_account_info,
            &fee_receiver_admin.to_account_info(),
            amount,
        )
    }

    pub fn swap_internal(ctx: Context<Swap>, data: SwapV2Args) -> Result<()> {
        ctx.accounts.check_raw_constraints(data.fee_token)?;
        check_deadline(data.deadline)?;
        ctx.accounts
            .express_relay_metadata
            .check_relayer_signer(ctx.accounts.relayer_signer.key)?;

        let (
            transfer_swap_fees,
            PostFeeSwapArgs {
                amount_searcher_after_fees,
                amount_user_after_fees,
            },
        ) = ctx.accounts.compute_swap_fees(&data)?;

        // We want the program to never fail in the CPI transfers after `check_enough_balances`.
        // This guarantees auction server than when a simulated transaction fails with the InsufficientUserFunds error,
        // the transaction was correct and executable other than the user having insufficient balance.
        // The checks above this line combined with `check_enough_balances` should guarantee that the CPIs will never fail
        // and `check_enough_balances` should be the last check.
        ctx.accounts.check_enough_balances(&data)?;

        ctx.accounts.transfer_swap_fees_cpi(&transfer_swap_fees)?;
        // Transfer tokens
        transfer_token_if_needed(
            &ctx.accounts.searcher_ta_mint_searcher,
            ctx.accounts.user_ata_mint_searcher.to_account_info(),
            &ctx.accounts.token_program_searcher,
            &ctx.accounts.searcher,
            &ctx.accounts.mint_searcher,
            amount_searcher_after_fees,
        )?;

        transfer_token_if_needed(
            &ctx.accounts.user_ata_mint_user,
            ctx.accounts.searcher_ta_mint_user.to_account_info(),
            &ctx.accounts.token_program_user,
            &ctx.accounts.user,
            &ctx.accounts.mint_user,
            amount_user_after_fees,
        )?;


        Ok(())
    }

    pub fn swap(ctx: Context<Swap>, data: SwapArgs) -> Result<()> {
        let data = ctx.accounts.convert_to_v2(&data);
        swap_internal(ctx, data)
    }

    pub fn swap_v2(ctx: Context<Swap>, data: SwapV2Args) -> Result<()> {
        swap_internal(ctx, data)
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Eq, PartialEq, Clone, Copy, Debug)]
pub struct InitializeArgs {
    pub split_router_default: u64,
    pub split_relayer:        u64,
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(init, payer = payer, space = RESERVE_EXPRESS_RELAY_METADATA, seeds = [SEED_METADATA], bump)]
    pub express_relay_metadata: Account<'info, ExpressRelayMetadata>,

    /// CHECK: this is just the admin's PK.
    pub admin: UncheckedAccount<'info>,

    /// CHECK: this is just the relayer's signer PK.
    pub relayer_signer: UncheckedAccount<'info>,

    /// CHECK: this is just a PK for the relayer to receive fees at.
    pub fee_receiver_relayer: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SetAdmin<'info> {
    pub admin: Signer<'info>,

    #[account(mut, seeds = [SEED_METADATA], bump, has_one = admin)]
    pub express_relay_metadata: Account<'info, ExpressRelayMetadata>,

    /// CHECK: this is just the new admin PK.
    pub admin_new: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct SetRelayer<'info> {
    pub admin: Signer<'info>,

    #[account(mut, seeds = [SEED_METADATA], bump, has_one = admin)]
    pub express_relay_metadata: Account<'info, ExpressRelayMetadata>,

    /// CHECK: this is the relayer public key which can be any arbitrary key.
    pub relayer_signer: UncheckedAccount<'info>,

    /// CHECK: this is the relayer fee receiver public key which can be any arbitrary key.
    pub fee_receiver_relayer: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct SetSecondaryRelayer<'info> {
    pub admin: Signer<'info>,

    #[account(mut, seeds = [SEED_METADATA], bump, has_one = admin)]
    pub express_relay_metadata: Account<'info, ExpressRelayMetadata>,

    /// CHECK: this is the secondary relayer public key which can be any arbitrary key.
    pub secondary_relayer_signer: UncheckedAccount<'info>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Eq, PartialEq, Clone, Copy, Debug)]
pub struct SetSplitsArgs {
    pub split_router_default: u64,
    pub split_relayer:        u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Eq, PartialEq, Clone, Copy, Debug)]
pub struct SetSwapPlatformFeeArgs {
    pub swap_platform_fee_bps: u64,
}

#[derive(Accounts)]
pub struct SetSplits<'info> {
    pub admin: Signer<'info>,

    #[account(mut, seeds = [SEED_METADATA], bump, has_one = admin)]
    pub express_relay_metadata: Account<'info, ExpressRelayMetadata>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Eq, PartialEq, Clone, Copy, Debug)]
pub struct SetRouterSplitArgs {
    pub split_router: u64,
}

#[derive(Accounts)]
pub struct SetRouterSplit<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(init_if_needed, payer = admin, space = RESERVE_EXPRESS_RELAY_CONFIG_ROUTER, seeds = [SEED_CONFIG_ROUTER, router.key().as_ref()], bump)]
    pub config_router: Account<'info, ConfigRouter>,

    #[account(seeds = [SEED_METADATA], bump, has_one = admin)]
    pub express_relay_metadata: Account<'info, ExpressRelayMetadata>,

    /// CHECK: this is just the router fee receiver PK.
    pub router: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Eq, PartialEq, Clone, Copy, Debug)]
pub struct SubmitBidArgs {
    // deadline as a unix timestamp in seconds
    pub deadline:   i64,
    pub bid_amount: u64,
}

#[derive(Accounts)]
pub struct SubmitBid<'info> {
    #[account(mut)]
    pub searcher: Signer<'info>,

    pub relayer_signer: Signer<'info>,

    /// CHECK: this is the permission key. Often the permission key refers to an on-chain account storing the opportunity; other times, it could refer to the 32 byte hash of identifying opportunity data. We include the permission as an account instead of putting it in the instruction data to save transaction size via caching in case of repeated use.
    pub permission: UncheckedAccount<'info>,

    /// CHECK: don't care what this looks like.
    #[account(mut)]
    pub router: UncheckedAccount<'info>,

    /// CHECK: Some routers might have an initialized ConfigRouter at the enforced PDA address which specifies a custom routing fee split. If the ConfigRouter is unitialized we will default to the routing fee split defined in the global ExpressRelayMetadata. We need to pass this account to check whether it exists and therefore there is a custom fee split.
    #[account(seeds = [SEED_CONFIG_ROUTER, router.key().as_ref()], bump)]
    pub config_router: UncheckedAccount<'info>,

    #[account(mut, seeds = [SEED_METADATA], bump, has_one = fee_receiver_relayer)]
    pub express_relay_metadata: Account<'info, ExpressRelayMetadata>,

    /// CHECK: this is just a PK for the relayer to receive fees at.
    #[account(mut)]
    pub fee_receiver_relayer: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,

    /// CHECK: this is the sysvar instructions account.
    #[account(address = sysvar_instructions::ID)]
    pub sysvar_instructions: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct CheckPermission<'info> {
    /// CHECK: this is the sysvar instructions account.
    #[account(address = sysvar_instructions::ID)]
    pub sysvar_instructions: UncheckedAccount<'info>,

    /// CHECK: this is the permission key. Often the permission key refers to an on-chain account storing the opportunity; other times, it could refer to the 32 byte hash of identifying opportunity data. We include the permission as an account instead of putting it in the instruction data to save transaction size via caching in case of repeated use.
    pub permission: UncheckedAccount<'info>,

    /// CHECK: this is the router address.
    pub router: UncheckedAccount<'info>,

    /// CHECK: this cannot be checked against ConfigRouter bc it may not be initialized.
    #[account(seeds = [SEED_CONFIG_ROUTER, router.key().as_ref()], bump)]
    pub config_router: UncheckedAccount<'info>,

    #[account(seeds = [SEED_METADATA], bump)]
    pub express_relay_metadata: Account<'info, ExpressRelayMetadata>,
}

#[derive(Accounts)]
pub struct WithdrawFees<'info> {
    pub admin: Signer<'info>,

    /// CHECK: this is just the PK where the fees should be sent.
    #[account(mut)]
    pub fee_receiver_admin: UncheckedAccount<'info>,

    #[account(mut, seeds = [SEED_METADATA], bump, has_one = admin)]
    pub express_relay_metadata: Account<'info, ExpressRelayMetadata>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Eq, PartialEq, Clone, Copy, Debug)]
pub enum FeeToken {
    Searcher,
    User,
}

/// For all swap instructions and contexts, the mint is defined with respect to the party that provides that token in the swap.
/// So `mint_searcher` refers to the token that the searcher provides in the swap,
/// and `mint_user` refers to the token that the user provides in the swap.
/// The `{X}_ta/ata_mint_{Y}` notation indicates the (associated) token account belonging to X for the mint of the token Y provides in the swap.
/// For example, `searcher_ta_mint_searcher` is the searcher's token account of the mint the searcher provides in the swap,
/// and `user_ata_mint_searcher` is the user's token account of the same mint.
#[derive(AnchorSerialize, AnchorDeserialize, Eq, PartialEq, Clone, Copy, Debug)]
pub struct SwapArgs {
    // deadline as a unix timestamp in seconds
    pub deadline:         i64,
    pub amount_searcher:  u64,
    pub amount_user:      u64,
    // The referral fee is specified in basis points
    pub referral_fee_bps: u16,
    // Token in which the fees will be paid
    pub fee_token:        FeeToken,
}

impl SwapArgs {
    pub fn convert_to_v2(&self, swap_platform_fee_bps: u64) -> SwapV2Args {
        SwapV2Args {
            deadline:              self.deadline,
            amount_searcher:       self.amount_searcher,
            amount_user:           self.amount_user,
            fee_token:             self.fee_token,
            referral_fee_ppm:      u64::from(self.referral_fee_bps) * FEE_BPS_TO_PPM,
            swap_platform_fee_ppm: swap_platform_fee_bps * FEE_BPS_TO_PPM,
        }
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Eq, PartialEq, Clone, Copy, Debug)]
pub struct SwapV2Args {
    /// deadline as a unix timestamp in seconds
    pub deadline:              i64,
    pub amount_searcher:       u64,
    pub amount_user:           u64,
    /// The referral fee is specified in parts per million
    pub referral_fee_ppm:      u64,
    /// Token in which the fees will be paid
    pub fee_token:             FeeToken,
    /// The platform fee is specified in parts per million
    pub swap_platform_fee_ppm: u64,
}

#[derive(Accounts)]
pub struct Swap<'info> {
    /// Searcher is the party that fulfills the quote request
    pub searcher: Signer<'info>,

    /// User is the party that requests the quote
    pub user: Signer<'info>,

    // Searcher accounts
    #[account(
        mut,
        token::mint = mint_searcher,
        token::authority = searcher,
        token::token_program = token_program_searcher
    )]
    pub searcher_ta_mint_searcher: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        token::mint = mint_user,
        token::authority = searcher,
        token::token_program = token_program_user
    )]
    pub searcher_ta_mint_user: Box<InterfaceAccount<'info, TokenAccount>>,

    // User accounts
    #[account(
        mut,
        associated_token::mint = mint_searcher,
        associated_token::authority = user,
        associated_token::token_program = token_program_searcher
    )]
    pub user_ata_mint_searcher: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        associated_token::mint = mint_user,
        associated_token::authority = user,
        associated_token::token_program = token_program_user
    )]
    pub user_ata_mint_user: Box<InterfaceAccount<'info, TokenAccount>>,

    // Fee receivers
    /// Router fee receiver token account: the referrer can provide an arbitrary receiver for the router fee
    /// CHECK: we check this account manually since it can be uninitialized if the fee is 0
    #[account(mut)]
    pub router_fee_receiver_ta: UncheckedAccount<'info>,

    /// CHECK: we check this account manually since it can be uninitialized if the fee is 0
    #[account(mut)]
    pub relayer_fee_receiver_ata: UncheckedAccount<'info>,

    /// CHECK: we check this account manually since it can be uninitialized if the fee is 0
    #[account(mut)]
    pub express_relay_fee_receiver_ata: UncheckedAccount<'info>,

    // Mints
    #[account(mint::token_program = token_program_searcher)]
    pub mint_searcher: Box<InterfaceAccount<'info, Mint>>,

    #[account(mint::token_program = token_program_user)]
    pub mint_user: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        mint::token_program = token_program_fee,
    )]
    /// CHECK: we check that this is set correctly based on the fee token manually since the V2 arguments are incompatible on the wire
    pub mint_fee: Box<InterfaceAccount<'info, Mint>>,

    // Token programs
    pub token_program_searcher: Interface<'info, TokenInterface>,
    pub token_program_user:     Interface<'info, TokenInterface>,

    /// CHECK: we check that this is set correctly based on the fee token manually since the V2 arguments are incompatible on the wire
    pub token_program_fee: Interface<'info, TokenInterface>,

    /// Express relay configuration
    #[account(seeds = [SEED_METADATA], bump)]
    pub express_relay_metadata: Box<Account<'info, ExpressRelayMetadata>>,

    pub relayer_signer: Signer<'info>,
}
