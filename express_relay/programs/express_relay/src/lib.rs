pub mod error;
pub mod state;
pub mod utils;

use anchor_lang::{prelude::*, system_program::System};
use anchor_lang::solana_program::sysvar::instructions as sysvar_instructions;
use solana_program::{serialize_utils::read_u16, sysvar::instructions::{get_instruction_relative, load_instruction_at_checked}};
use anchor_syn::codegen::program::common::sighash;
use anchor_spl::token::Token;
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

    pub fn permission(ctx: Context<Permission>, data: PermissionArgs) -> Result<()> {
        if data.deadline < Clock::get()?.unix_timestamp as u64 {
            return err!(ErrorCode::DeadlinePassed);
        }

        // check that not cpi
        let instruction = get_instruction_relative(0, &ctx.accounts.sysvar_instructions.to_account_info())?;
        if instruction.program_id != crate::id() {
            return err!(ErrorCode::InvalidCPIPermission);
        }

        // handle bid payment
        let bid_amount = data.bid_amount;
        let searcher = &ctx.accounts.searcher;
        let rent_searcher = Rent::default().minimum_balance(0);
        if bid_amount + rent_searcher > searcher.lamports() {
            return err!(ErrorCode::InsufficientSearcherFunds);
        }

        let express_relay_metadata = &ctx.accounts.express_relay_metadata;
        let split_relayer = express_relay_metadata.split_relayer;
        let split_protocol_default = express_relay_metadata.split_protocol_default;

        let split_protocol: u64;
        let protocol_config = &ctx.accounts.protocol_config;
        let protocol_config_account_info = protocol_config.to_account_info();
        if protocol_config_account_info.data_len() > 0 {
            let account_data = &mut &**protocol_config_account_info.try_borrow_data()?;
            let protocol_config_data = ConfigProtocol::try_deserialize(account_data)?;
            split_protocol = protocol_config_data.split;
        } else {
            split_protocol = split_protocol_default;
        }

        let fee_protocol = bid_amount * split_protocol / FEE_SPLIT_PRECISION;
        if fee_protocol > bid_amount {
            return err!(ErrorCode::FeesHigherThanBid);
        }

        let fee_relayer = bid_amount.saturating_sub(fee_protocol) * split_relayer / FEE_SPLIT_PRECISION;
        if fee_relayer.checked_add(fee_protocol).ok_or(ProgramError::ArithmeticOverflow)? > bid_amount {
            return err!(ErrorCode::FeesHigherThanBid);
        }

        let protocol_fee_receiver = &ctx.accounts.fee_receiver_protocol;
        let balance_protocol_fee_receiver = protocol_fee_receiver.lamports();
        let rent_protocol_fee_receiver = Rent::default().minimum_balance(0);
        if balance_protocol_fee_receiver+fee_protocol < rent_protocol_fee_receiver {
            return err!(ErrorCode::InsufficientProtocolFeeReceiverFundsForRent);
        }

        let relayer_fee_receiver = &ctx.accounts.fee_receiver_relayer;
        let balance_relayer_fee_receiver = relayer_fee_receiver.lamports();
        let rent_relayer_fee_receiver = Rent::default().minimum_balance(0);
        if balance_relayer_fee_receiver+fee_relayer < rent_relayer_fee_receiver {
            return err!(ErrorCode::InsufficientRelayerFeeReceiverFundsForRent);
        }

        transfer_lamports_cpi(
            &searcher.to_account_info(),
            &protocol_fee_receiver.to_account_info(),
            fee_protocol,
            ctx.accounts.system_program.to_account_info()
        )?;
        transfer_lamports_cpi(
            &searcher.to_account_info(),
            &relayer_fee_receiver.to_account_info(),
            fee_relayer,
            ctx.accounts.system_program.to_account_info()
        )?;
        // send the remaining balance from the bid to the express relay metadata account
        transfer_lamports_cpi(
            &searcher.to_account_info(),
            &express_relay_metadata.to_account_info(),
            bid_amount.saturating_sub(fee_protocol).saturating_sub(fee_relayer),
            ctx.accounts.system_program.to_account_info()
        )?;

        Ok(())
    }

    pub fn check_permission(ctx: Context<CheckPermission>) -> Result<()> {
        let num_instructions = read_u16(&mut 0, &ctx.accounts.sysvar_instructions.data.borrow()).map_err(|_| ProgramError::InvalidInstructionData)?;
        for index in 0..num_instructions {
            let ix = load_instruction_at_checked(index.into(), &ctx.accounts.sysvar_instructions)?;

            if ix.program_id != crate::id() {
                continue;
            }
            let expected_discriminator = sighash("global", "permission");
            if ix.data[0..8] != expected_discriminator {
                continue;
            }

            if ix.accounts[2].pubkey != *ctx.accounts.permission.key {
                continue;
            }

            if ix.accounts[3].pubkey == *ctx.accounts.protocol.key {
                return Ok(());
            }
        }

        return err!(ErrorCode::InvalidPermissioning);
    }

    pub fn withdraw_fees(ctx: Context<WithdrawFees>) -> Result<()> {
        let express_relay_metadata = &ctx.accounts.express_relay_metadata;
        let fee_receiver_admin = &ctx.accounts.fee_receiver_admin;

        let express_relay_metadata_account_info = express_relay_metadata.to_account_info();
        let rent_express_relay_metadata = Rent::get()?.minimum_balance(express_relay_metadata_account_info.data_len());

        if express_relay_metadata_account_info.lamports() <= rent_express_relay_metadata {
            return err!(ErrorCode::InsufficientWithdrawalFunds);
        }

        let amount = express_relay_metadata_account_info.lamports() - rent_express_relay_metadata;
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

    /// CHECK: this is just the PK for the admin to sign from
    pub admin: UncheckedAccount<'info>,

    /// CHECK: this is just the PK for the relayer to sign from
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

    /// CHECK: this is just the PK for the relayer to sign from
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
pub struct PermissionArgs {
    pub deadline: u64,
    pub bid_amount: u64,
}

#[derive(Accounts)]
pub struct Permission<'info> {
    #[account(mut)]
    pub searcher: Signer<'info>,

    pub relayer_signer: Signer<'info>,

    /// CHECK: this is the permission_key
    pub permission: UncheckedAccount<'info>,

    /// CHECK: this is the protocol/router address
    pub protocol: UncheckedAccount<'info>,

    /// CHECK: this cannot be checked against ConfigProtocol bc it may not be initialized bc anchor :(
    #[account(seeds = [SEED_CONFIG_PROTOCOL, protocol.key().as_ref()], bump)]
    pub protocol_config: UncheckedAccount<'info>,

    /// CHECK: this is just a PK for the relayer to receive fees at
    #[account(mut)]
    pub fee_receiver_relayer: UncheckedAccount<'info>,

	/// CHECK: don't care what this PDA looks like
    #[account(mut, seeds = [SEED_EXPRESS_RELAY_FEES], seeds::program = protocol.key(), bump)]
    pub fee_receiver_protocol: UncheckedAccount<'info>,

    #[account(mut, seeds = [SEED_METADATA], bump, has_one = relayer_signer, has_one = fee_receiver_relayer)]
    pub express_relay_metadata: Account<'info, ExpressRelayMetadata>,

    pub system_program: Program<'info, System>,

    pub token_program: Program<'info, Token>,

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
