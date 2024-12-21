pub mod error;
pub mod sdk;
pub mod state;
pub mod token;
pub mod utils;

use {
    crate::{
        error::ErrorCode,
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
    use {
        super::*,
        token::{
            check_receiver_associated_token_account,
            check_receiver_token_account,
            transfer_token_if_needed,
        },
    };

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

    /// Submits a bid for a particular (permission, router) pair and distributes bids according to splits.
    pub fn submit_bid(ctx: Context<SubmitBid>, data: SubmitBidArgs) -> Result<()> {
        if data.deadline < Clock::get()?.unix_timestamp {
            return err!(ErrorCode::DeadlinePassed);
        }

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

    pub fn swap(ctx: Context<Swap>, data: SwapArgs) -> Result<()> {
        let (input_after_fees, output_after_fees, send_fee_args): (u64, u64, SendSwapFeeArgs) =
            match data.fee_token {
                FeeToken::Input => (
                    data.amount_input,
                    data.amount_output,
                    SendSwapFeeArgs {
                        fee_express_relay: 0,
                        fee_relayer:       0,
                        fee_router:        0,
                        mint:              ctx.accounts.mint_input.clone(),
                        token_program:     ctx.accounts.token_program_input.clone(),
                        from:              ctx.accounts.searcher_input_ta.clone(),
                        authority:         ctx.accounts.searcher.clone(),
                    },
                ),
                FeeToken::Output => (
                    data.amount_input,
                    data.amount_output,
                    SendSwapFeeArgs {
                        fee_express_relay: 0,
                        fee_relayer:       0,
                        fee_router:        0,
                        from:              ctx.accounts.trader_output_ata.clone(),
                        authority:         ctx.accounts.trader.clone(),
                        mint:              ctx.accounts.mint_output.clone(),
                        token_program:     ctx.accounts.token_program_output.clone(),
                    },
                ),
            };


        // Check ATAs
        check_receiver_associated_token_account(
            &ctx.accounts.express_relay_fee_receiver_ata,
            &ctx.accounts.express_relay_metadata.key(),
            &send_fee_args.mint,
            &send_fee_args.token_program,
        )?;
        check_receiver_associated_token_account(
            &ctx.accounts.relayer_fee_receiver_ata,
            &ctx.accounts.express_relay_metadata.relayer_signer,
            &send_fee_args.mint,
            &send_fee_args.token_program,
        )?;
        check_receiver_token_account(
            &ctx.accounts.router_fee_receiver_ta,
            &send_fee_args.mint,
            &send_fee_args.token_program,
        )?;

        // Send fees
        transfer_token_if_needed(
            &send_fee_args.from,
            &ctx.accounts.express_relay_fee_receiver_ata,
            &send_fee_args.token_program,
            &send_fee_args.authority,
            &send_fee_args.mint,
            send_fee_args.fee_express_relay,
        )?;

        transfer_token_if_needed(
            &send_fee_args.from,
            &ctx.accounts.relayer_fee_receiver_ata,
            &send_fee_args.token_program,
            &send_fee_args.authority,
            &send_fee_args.mint,
            send_fee_args.fee_relayer,
        )?;

        transfer_token_if_needed(
            &send_fee_args.from,
            &ctx.accounts.router_fee_receiver_ta,
            &send_fee_args.token_program,
            &send_fee_args.authority,
            &send_fee_args.mint,
            send_fee_args.fee_router,
        )?;

        // Transfer tokens
        transfer_token_if_needed(
            &ctx.accounts.searcher_input_ta,
            &ctx.accounts.trader_input_ata,
            &ctx.accounts.token_program_input,
            &ctx.accounts.searcher,
            &ctx.accounts.mint_input,
            input_after_fees,
        )?;

        transfer_token_if_needed(
            &ctx.accounts.trader_output_ata,
            &ctx.accounts.searcher_output_ta,
            &ctx.accounts.token_program_output,
            &ctx.accounts.trader,
            &ctx.accounts.mint_output,
            output_after_fees,
        )?;


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
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(mut, seeds = [SEED_METADATA], bump, has_one = admin)]
    pub express_relay_metadata: Account<'info, ExpressRelayMetadata>,

    /// CHECK: this is just the new admin PK.
    pub admin_new: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct SetRelayer<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(mut, seeds = [SEED_METADATA], bump, has_one = admin)]
    pub express_relay_metadata: Account<'info, ExpressRelayMetadata>,

    /// CHECK: this is just the relayer's signer PK.
    pub relayer_signer: UncheckedAccount<'info>,

    /// CHECK: this is just a PK for the relayer to receive fees at.
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

    /// CHECK: this is just the router fee receiver PK.
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

    /// CHECK: don't care what this looks like.
    #[account(mut)]
    pub router: UncheckedAccount<'info>,

    /// CHECK: Some routers might have an initialized ConfigRouter at the enforced PDA address which specifies a custom routing fee split. If the ConfigRouter is unitialized we will default to the routing fee split defined in the global ExpressRelayMetadata. We need to pass this account to check whether it exists and therefore there is a custom fee split.
    #[account(seeds = [SEED_CONFIG_ROUTER, router.key().as_ref()], bump)]
    pub config_router: UncheckedAccount<'info>,

    #[account(mut, seeds = [SEED_METADATA], bump, has_one = relayer_signer, has_one = fee_receiver_relayer)]
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
    #[account(mut)]
    pub admin: Signer<'info>,

    /// CHECK: this is just the PK where the fees should be sent.
    #[account(mut)]
    pub fee_receiver_admin: UncheckedAccount<'info>,

    #[account(mut, seeds = [SEED_METADATA], bump, has_one = admin)]
    pub express_relay_metadata: Account<'info, ExpressRelayMetadata>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Eq, PartialEq, Clone, Copy, Debug)]
pub enum FeeToken {
    Input,
    Output,
}

/// For all swap instructions and contexts, input and output are defined with respect to the searcher
/// So mint_input refers to the token that the searcher provides to the trader
/// mint_output refers to the token that the searcher receives from the trader
/// This choice is made to minimize confusion for the searchers, who are more likely to parse the program
#[derive(AnchorSerialize, AnchorDeserialize, Eq, PartialEq, Clone, Copy, Debug)]
pub struct SwapArgs {
    pub amount_input:  u64,
    pub amount_output: u64,
    // The referral fee is specified in parts per million (e.g. 100 = 1 bp)
    pub referral_fee:  u64,
    // Token in which the fees will be paid
    pub fee_token:     FeeToken,
}

pub struct SendSwapFeeArgs<'info> {
    /// Amount to which the fee will be applied
    pub fee_express_relay: u64,
    pub fee_relayer:       u64,
    pub fee_router:        u64,
    pub from:              InterfaceAccount<'info, TokenAccount>,
    pub authority:         Signer<'info>,
    pub mint:              InterfaceAccount<'info, Mint>,
    pub token_program:     Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
#[instruction(data: Box<SwapArgs>)]
pub struct Swap<'info> {
    /// Searcher is the party that sends the input token and receives the output token
    pub searcher: Signer<'info>,

    /// Trader is the party that sends the output token and receives the input token
    pub trader: Signer<'info>,

    // Searcher accounts
    #[account(
        mut,
        token::mint = mint_input,
        token::authority = searcher,
        token::token_program = token_program_input
    )]
    pub searcher_input_ta: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        token::mint = mint_output,
        token::authority = searcher,
        token::token_program = token_program_output
    )]
    pub searcher_output_ta: InterfaceAccount<'info, TokenAccount>,

    // Trader accounts
    #[account(
        mut,
        associated_token::mint = mint_input,
        associated_token::authority = trader,
        associated_token::token_program = token_program_input
    )]
    pub trader_input_ata: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        associated_token::mint = mint_output,
        associated_token::authority = trader,
        associated_token::token_program = token_program_output
    )]
    pub trader_output_ata: InterfaceAccount<'info, TokenAccount>,

    // Fee receivers
    #[account(mut)]
    pub router_fee_receiver_ta: InterfaceAccount<'info, TokenAccount>,

    #[account(mut,
        token::authority = express_relay_metadata.relayer_signer,
    )]
    pub relayer_fee_receiver_ata: InterfaceAccount<'info, TokenAccount>,

    #[account(mut,
        token::authority = express_relay_metadata.key(),
    )]
    pub express_relay_fee_receiver_ata: InterfaceAccount<'info, TokenAccount>,

    // Mints
    #[account(mint::token_program = token_program_input)]
    pub mint_input: InterfaceAccount<'info, Mint>,

    #[account(mint::token_program = token_program_output)]
    pub mint_output: InterfaceAccount<'info, Mint>,

    // Token programs
    pub token_program_input:  Interface<'info, TokenInterface>,
    pub token_program_output: Interface<'info, TokenInterface>,

    /// Express relay configuration
    #[account(seeds = [SEED_METADATA], bump)]
    pub express_relay_metadata: Account<'info, ExpressRelayMetadata>,
}
