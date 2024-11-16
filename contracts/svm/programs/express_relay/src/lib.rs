pub mod error;
pub mod sdk;
pub mod state;
pub mod utils;

use {
    crate::{
        cpi::accounts::CheckPermission as CheckPermissionCPI,
        error::ErrorCode,
        program::ExpressRelay,
        sdk::cpi::check_permission_cpi,
        state::*,
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

    pub fn set_splits(ctx: Context<SetSplits>, data: SetSplitsArgs) -> Result<()> {
        validate_fee_split(data.split_router_default)?;
        validate_fee_split(data.split_relayer)?;

        let express_relay_metadata_data = &mut ctx.accounts.express_relay_metadata;

        express_relay_metadata_data.split_router_default = data.split_router_default;
        express_relay_metadata_data.split_relayer = data.split_relayer;

        Ok(())
    }

    pub fn set_router_split(ctx: Context<SetRouterSplit>, data: SetRouterSplitArgs) -> Result<()> {
        validate_fee_split(data.split_router)?;

        ctx.accounts.config_router.router = *ctx.accounts.router.key;
        ctx.accounts.config_router.split = data.split_router;

        Ok(())
    }

    /// Submits a bid for a particular (permission, router) pair and distributes bids according to splits
    pub fn submit_bid(ctx: Context<SubmitBid>, data: SubmitBidArgs) -> Result<()> {
        if data.deadline < Clock::get()?.unix_timestamp {
            return err!(ErrorCode::DeadlinePassed);
        }

        // check that not cpi
        if get_stack_height() > TRANSACTION_LEVEL_STACK_HEIGHT {
            return err!(ErrorCode::InvalidCPISubmitBid);
        }

        // check "no reentrancy"--SubmitBid instruction only used once in transaction
        // this is done to prevent an exploit where a searcher submits a transaction with multiple SubmitBid instructions with different permission keys
        // that could allow the searcher to win the right to perform the transaction if they won just one of the auctions
        let matching_ixs = get_matching_submit_bid_instructions(
            ctx.accounts.sysvar_instructions.to_account_info(),
            None,
        )?;
        if matching_ixs.len() > 1 {
            return err!(ErrorCode::MultiplePermissions);
        }

        handle_bid_payment(ctx, data.bid_amount)
    }

    /// Checks if permissioning exists for a particular (permission, router) pair within the same transaction
    /// Permissioning takes the form of a SubmitBid instruction with matching permission and router accounts
    /// Returns the fees paid to the router in the matching instructions
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

    pub fn swap(ctx: Context<Swap>, data: SwapArgs) -> Result<()> {
        // check permission
        let check_permission_accounts = CheckPermissionCPI {
            sysvar_instructions:    ctx.accounts.sysvar_instructions.to_account_info(),
            permission:             ctx.accounts.permission.to_account_info(),
            router:                 ctx.accounts.router.to_account_info(),
            config_router:          ctx.accounts.config_router.to_account_info(),
            express_relay_metadata: ctx.accounts.express_relay_metadata.to_account_info(),
        };
        check_permission_cpi(
            check_permission_accounts,
            ctx.accounts.express_relay_program.to_account_info(),
        )?;

        // this is just a defensive programming measure, since an excessive referral fee should fail the checked_sub calls below
        if data.referral_fee_ppm > 1_000_000 {
            return err!(ErrorCode::InvalidReferralFee);
        }

        validate_ata(
            &ctx.accounts.ta_input_trader.key(),
            &ctx.accounts.trader.key(),
            &ctx.accounts.mint_input.key(),
        )?;
        validate_ata(
            &ctx.accounts.ta_input_router.key(),
            &ctx.accounts.router.key(),
            &ctx.accounts.mint_input.key(),
        )?;
        validate_ata(
            &ctx.accounts.ta_output_router.key(),
            &ctx.accounts.router.key(),
            &ctx.accounts.mint_output.key(),
        )?;

        let (fees_input, fees_output) = match data.referral_fee_input {
            true => {
                let fees_referral = compute_and_transfer_fee(
                    &ctx.accounts.ta_input_searcher.to_account_info(),
                    &ctx.accounts.ta_input_router.to_account_info(),
                    &ctx.accounts.token_program_input.to_account_info(),
                    &ctx.accounts.router.to_account_info(),
                    &ctx.accounts.mint_input,
                    data.amount_input,
                    data.referral_fee_ppm,
                )?;

                (fees_referral, 0)
            }
            false => {
                let fees_referral = compute_and_transfer_fee(
                    &ctx.accounts.ta_output_trader.to_account_info(),
                    &ctx.accounts.ta_output_router.to_account_info(),
                    &ctx.accounts.token_program_output.to_account_info(),
                    &ctx.accounts.trader.to_account_info(),
                    &ctx.accounts.mint_output,
                    data.amount_output,
                    data.referral_fee_ppm,
                )?;

                (0, fees_referral)
            }
        };

        let amount_to_trader = data
            .amount_input
            .checked_sub(fees_input)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        if amount_to_trader > 0 {
            transfer_spl(
                &ctx.accounts.ta_input_searcher.to_account_info(),
                &ctx.accounts.ta_input_trader.to_account_info(),
                &ctx.accounts.token_program_input.to_account_info(),
                &ctx.accounts.searcher.to_account_info(),
                &ctx.accounts.mint_input,
                amount_to_trader,
            )?;
        }

        let amount_to_searcher = data
            .amount_output
            .checked_sub(fees_output)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        if amount_to_searcher > 0 {
            transfer_spl(
                &ctx.accounts.ta_output_trader.to_account_info(),
                &ctx.accounts.ta_output_searcher.to_account_info(),
                &ctx.accounts.token_program_output.to_account_info(),
                &ctx.accounts.trader.to_account_info(),
                &ctx.accounts.mint_output,
                amount_to_searcher,
            )?;
        }

        Ok(())
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

    /// CHECK: this is just the admin's PK
    pub admin: UncheckedAccount<'info>,

    /// CHECK: this is just the relayer's signer PK
    pub relayer_signer: UncheckedAccount<'info>,

    /// CHECK: this is just a PK for the relayer to receive fees at
    pub fee_receiver_relayer: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SetAdmin<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(mut, seeds = [SEED_METADATA], bump, has_one = admin)]
    pub express_relay_metadata: Account<'info, ExpressRelayMetadata>,

    /// CHECK: this is just the new admin PK
    pub admin_new: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct SetRelayer<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(mut, seeds = [SEED_METADATA], bump, has_one = admin)]
    pub express_relay_metadata: Account<'info, ExpressRelayMetadata>,

    /// CHECK: this is just the relayer's signer PK
    pub relayer_signer: UncheckedAccount<'info>,

    /// CHECK: this is just a PK for the relayer to receive fees at
    pub fee_receiver_relayer: UncheckedAccount<'info>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Eq, PartialEq, Clone, Copy, Debug)]
pub struct SetSplitsArgs {
    pub split_router_default: u64,
    pub split_relayer:        u64,
}

#[derive(Accounts)]
pub struct SetSplits<'info> {
    #[account(mut)]
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

    /// CHECK: this is just the router fee receiver PK
    pub router: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Eq, PartialEq, Clone, Copy, Debug)]
pub struct SubmitBidArgs {
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

    /// CHECK: don't care what this looks like
    #[account(mut)]
    pub router: UncheckedAccount<'info>,

    /// CHECK: Some routers might have an initialized ConfigRouter at the enforced PDA address which specifies a custom routing fee split. If the ConfigRouter is unitialized we will default to the routing fee split defined in the global ExpressRelayMetadata. We need to pass this account to check whether it exists and therefore there is a custom fee split.
    #[account(seeds = [SEED_CONFIG_ROUTER, router.key().as_ref()], bump)]
    pub config_router: UncheckedAccount<'info>,

    #[account(mut, seeds = [SEED_METADATA], bump, has_one = relayer_signer, has_one = fee_receiver_relayer)]
    pub express_relay_metadata: Account<'info, ExpressRelayMetadata>,

    /// CHECK: this is just a PK for the relayer to receive fees at
    #[account(mut)]
    pub fee_receiver_relayer: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,

    /// CHECK: this is the sysvar instructions account
    #[account(address = sysvar_instructions::ID)]
    pub sysvar_instructions: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct CheckPermission<'info> {
    /// CHECK: this is the sysvar instructions account
    #[account(address = sysvar_instructions::ID)]
    pub sysvar_instructions: UncheckedAccount<'info>,

    /// CHECK: this is the permission key. Often the permission key refers to an on-chain account storing the opportunity; other times, it could refer to the 32 byte hash of identifying opportunity data. We include the permission as an account instead of putting it in the instruction data to save transaction size via caching in case of repeated use.
    pub permission: UncheckedAccount<'info>,

    /// CHECK: this is the router address
    pub router: UncheckedAccount<'info>,

    /// CHECK: this cannot be checked against ConfigRouter bc it may not be initialized.
    #[account(seeds = [SEED_CONFIG_ROUTER, router.key().as_ref()], bump)]
    pub config_router: UncheckedAccount<'info>,

    #[account(seeds = [SEED_METADATA], bump)]
    pub express_relay_metadata: Account<'info, ExpressRelayMetadata>,
}

#[derive(Accounts)]
pub struct WithdrawFees<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    /// CHECK: this is just the PK where the fees should be sent
    #[account(mut)]
    pub fee_receiver_admin: UncheckedAccount<'info>,

    #[account(mut, seeds = [SEED_METADATA], bump, has_one = admin)]
    pub express_relay_metadata: Account<'info, ExpressRelayMetadata>,
}

/// For all swap instructions and contexts, input and output are defined with respect to the searcher
/// So mint_input refers to the token that the searcher provides to the trader
/// mint_output refers to the token that the searcher receives from the trader
/// This choice is made to minimize confusion for the searchers, who are more likely to parse the program
#[derive(AnchorSerialize, AnchorDeserialize, Eq, PartialEq, Clone, Copy, Debug)]
pub struct SwapArgs {
    pub amount_input:       u64,
    pub amount_output:      u64,
    pub nonce:              u64,
    // The referral fee is specified in parts per million (e.g. 100 = 1 bp)
    pub referral_fee_ppm:   u64,
    // Whether the referral fee should be applied to the input token
    pub referral_fee_input: bool,
}

#[derive(Accounts)]
#[instruction(data: Box<SwapArgs>)]
pub struct Swap<'info> {
    #[account(mut)]
    pub searcher: Signer<'info>,

    pub trader: Signer<'info>,

    #[account(seeds = [
        SEED_SWAP,
        trader.key().as_ref(),
        mint_input.key().as_ref(),
        mint_output.key().as_ref(),
        &data.nonce.to_le_bytes(),
    ], bump)]
    pub permission: AccountInfo<'info>,

    /// CHECK: don't care what this looks like
    #[account(mut)]
    pub router: UncheckedAccount<'info>,

    /// CHECK: this cannot be checked against ConfigRouter bc it may not be initialized bc anchor. we need to check this config even when unused to make sure unique fee splits don't exist
    #[account(seeds = [SEED_CONFIG_ROUTER, router.key().as_ref()], bump)]
    pub config_router: UncheckedAccount<'info>,

    #[account(mut, seeds = [SEED_METADATA], bump)]
    pub express_relay_metadata: Account<'info, ExpressRelayMetadata>,

    #[account(mint::token_program = token_program_input)]
    pub mint_input: InterfaceAccount<'info, Mint>,

    #[account(mint::token_program = token_program_output)]
    pub mint_output: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        token::mint = mint_input,
        token::authority = searcher,
        token::token_program = token_program_input
    )]
    pub ta_input_searcher: InterfaceAccount<'info, TokenAccount>,

    #[account(
        init_if_needed,
        payer = searcher,
        token::mint = mint_output,
        token::authority = searcher,
        token::token_program = token_program_output
    )]
    pub ta_output_searcher: InterfaceAccount<'info, TokenAccount>,

    // This account may not be initialized, and it should be set to the trader's ATA to prevent the trader from
    // receiving the input tokens in an arbitrary token account. We perform this check within the instruction.
    #[account(
        init_if_needed,
        payer = searcher,
        token::mint = mint_input,
        token::authority = trader,
        token::token_program = token_program_input
    )]
    pub ta_input_trader: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        token::mint = mint_output,
        token::authority = trader,
        token::token_program = token_program_output
    )]
    pub ta_output_trader: InterfaceAccount<'info, TokenAccount>,

    // This account may not be initialized, and it should be set to the router's ATA to prevent the router from
    // receiving input tokens in an arbitrary token account. We perform this check within the instruction.
    #[account(
        init_if_needed,
        payer = searcher,
        token::mint = mint_input,
        token::authority = router,
        token::token_program = token_program_input
    )]
    pub ta_input_router: InterfaceAccount<'info, TokenAccount>,

    // This account may not be initialized, and it should be set to the router's ATA to prevent the router from
    // receiving output tokens in an arbitrary token account. We perform this check within the instruction.
    #[account(
        init_if_needed,
        payer = searcher,
        token::mint = mint_output,
        token::authority = router,
        token::token_program = token_program_output
    )]
    pub ta_output_router: InterfaceAccount<'info, TokenAccount>,

    pub express_relay_program: Program<'info, ExpressRelay>,

    pub token_program_input: Interface<'info, TokenInterface>,

    pub token_program_output: Interface<'info, TokenInterface>,

    pub system_program: Program<'info, System>,

    /// CHECK: this is the sysvar instructions account
    #[account(address = sysvar_instructions::ID)]
    pub sysvar_instructions: UncheckedAccount<'info>,
}
