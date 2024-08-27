pub mod error;
pub mod state;
pub mod utils;

use anchor_lang::{prelude::*, system_program::System};
use anchor_lang::solana_program::sysvar::instructions as sysvar_instructions;
use solana_program::instruction::{get_stack_height, TRANSACTION_LEVEL_STACK_HEIGHT};
use crate::{
    error::ErrorCode,
    state::*,
    utils::*,
};

declare_id!("GwEtasTAxdS9neVE4GPUpcwR7DB7AizntQSPcG36ubZM");

#[program]
pub mod express_relay {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>, data: InitializeArgs) -> Result<()> {
        validate_fee_split(data.split_protocol_default)?;
        validate_fee_split(data.split_relayer)?;

        let express_relay_metadata_data = &mut ctx.accounts.express_relay_metadata;

        express_relay_metadata_data.admin = *ctx.accounts.admin.key;
        express_relay_metadata_data.relayer_signer = *ctx.accounts.relayer_signer.key;
        express_relay_metadata_data.fee_receiver_relayer = *ctx.accounts.fee_receiver_relayer.key;
        express_relay_metadata_data.split_protocol_default = data.split_protocol_default;
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
        validate_fee_split(data.split_protocol_default)?;
        validate_fee_split(data.split_relayer)?;

        let express_relay_metadata_data = &mut ctx.accounts.express_relay_metadata;

        express_relay_metadata_data.split_protocol_default = data.split_protocol_default;
        express_relay_metadata_data.split_relayer = data.split_relayer;

        Ok(())
    }

    pub fn set_protocol_split(ctx: Context<SetProtocolSplit>, data: SetProtocolSplitArgs) -> Result<()> {
        validate_fee_split(data.split_protocol)?;

        ctx.accounts.protocol_config.protocol = *ctx.accounts.protocol.key;
        ctx.accounts.protocol_config.split = data.split_protocol;

        Ok(())
    }

    // Submits a bid for a particular (protocol, permission) pair and distributes bids according to splits
    pub fn submit_bid(ctx: Context<SubmitBid>, data: SubmitBidArgs) -> Result<()> {
        if data.deadline < Clock::get()?.unix_timestamp {
            return err!(ErrorCode::DeadlinePassed);
        }

        // check that not cpi
        if get_stack_height() > TRANSACTION_LEVEL_STACK_HEIGHT {
            return err!(ErrorCode::InvalidCPISubmitBid);
        }

        // check "no reentrancy"--submit_bid instruction only used once in transaction
        // this is done to prevent an exploit where a searcher submits a transaction with multiple submit_bid instructions with different permission keys
        // that would allow the searcher to win the right to perform the transaction if they won just one of the auctions
        let permission_count = num_permissions_in_tx(ctx.accounts.sysvar_instructions.clone(), None, None)?;
        if permission_count > 1 {
            return err!(ErrorCode::MultiplePermissions);
        }

        handle_bid_payment(ctx, data.bid_amount)
    }

    // Checks if permissioning exists for a particular (protocol, permission) pair within the same transaction
    // Permissioning takes the form of a submit_bid instruction with matching protocol and permission accounts
    pub fn check_permission(ctx: Context<CheckPermission>) -> Result<()> {
        let num_permissions = num_permissions_in_tx(ctx.accounts.sysvar_instructions.clone(), Some(*ctx.accounts.permission.key), Some(*ctx.accounts.protocol.key))?;

        if num_permissions == 0 {
            return err!(ErrorCode::MissingPermission);
        }

        Ok(())
    }

    pub fn withdraw_fees(ctx: Context<WithdrawFees>) -> Result<()> {
        let express_relay_metadata = &ctx.accounts.express_relay_metadata;
        let fee_receiver_admin = &ctx.accounts.fee_receiver_admin;

        let express_relay_metadata_account_info = express_relay_metadata.to_account_info();
        let rent_express_relay_metadata = Rent::get()?.minimum_balance(express_relay_metadata_account_info.data_len());

        let amount = express_relay_metadata_account_info.lamports().saturating_sub(rent_express_relay_metadata);
        if amount == 0 {
            return Ok(());
        }
        transfer_lamports(&express_relay_metadata_account_info, &fee_receiver_admin.to_account_info(), amount)
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Eq, PartialEq, Clone, Copy, Debug)]
pub struct InitializeArgs {
    pub split_protocol_default: u64,
    pub split_relayer: u64
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
    pub split_protocol_default: u64,
    pub split_relayer: u64,
}

#[derive(Accounts)]
pub struct SetSplits<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(mut, seeds = [SEED_METADATA], bump, has_one = admin)]
    pub express_relay_metadata: Account<'info, ExpressRelayMetadata>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Eq, PartialEq, Clone, Copy, Debug)]
pub struct SetProtocolSplitArgs {
    pub split_protocol: u64,
}

#[derive(Accounts)]
pub struct SetProtocolSplit<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(init_if_needed, payer = admin, space = RESERVE_EXPRESS_RELAY_CONFIG_PROTOCOL, seeds = [SEED_CONFIG_PROTOCOL, protocol.key().as_ref()], bump)]
    pub protocol_config: Account<'info, ConfigProtocol>,

    #[account(seeds = [SEED_METADATA], bump, has_one = admin)]
    pub express_relay_metadata: Account<'info, ExpressRelayMetadata>,

    /// CHECK: this is just the protocol fee receiver PK
    pub protocol: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Eq, PartialEq, Clone, Copy, Debug)]
pub struct SubmitBidArgs {
    pub deadline: i64,
    pub bid_amount: u64,
}

#[derive(Accounts)]
pub struct SubmitBid<'info> {
    #[account(mut)]
    pub searcher: Signer<'info>,

    pub relayer_signer: Signer<'info>,

    /// CHECK: this is the permission_key
    pub permission: UncheckedAccount<'info>,

    /// CHECK: this is the protocol/router address
    pub protocol: UncheckedAccount<'info>,

    /// CHECK: this cannot be checked against ConfigProtocol bc it may not be initialized bc anchor. we need to check this config even when unused to make sure unique fee splits don't exist
    #[account(seeds = [SEED_CONFIG_PROTOCOL, protocol.key().as_ref()], bump)]
    pub protocol_config: UncheckedAccount<'info>,

    /// CHECK: this is just a PK for the relayer to receive fees at
    #[account(mut)]
    pub fee_receiver_relayer: UncheckedAccount<'info>,

    /// CHECK: don't care what this looks like; if PDA, validate within program logic
    #[account(mut)]
    pub fee_receiver_protocol: UncheckedAccount<'info>,

    #[account(mut, seeds = [SEED_METADATA], bump, has_one = relayer_signer, has_one = fee_receiver_relayer)]
    pub express_relay_metadata: Account<'info, ExpressRelayMetadata>,

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

    /// CHECK: this is the permission_key
    pub permission: UncheckedAccount<'info>,

    /// CHECK: this is the protocol/router address
    pub protocol: UncheckedAccount<'info>,
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
