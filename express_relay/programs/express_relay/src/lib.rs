pub mod error;
pub mod state;
pub mod utils;

use anchor_lang::{prelude::*, system_program::System};
use anchor_lang::solana_program::sysvar::instructions as tx_instructions;
use solana_program::{serialize_utils::read_u16, sysvar::instructions::{load_current_index_checked, load_instruction_at_checked}};
use crate::{
    error::ExpressRelayError,
    state::*,
    utils::*,
};

declare_id!("7L8f7kMv4swkFPeisT4qd137FEnmmRy4HCWh2YAbrsNh");

#[program]
pub mod express_relay {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>, data: InitializeArgs) -> Result<()> {
        validate_fee_split(data.split_protocol_default)?;
        validate_fee_split(data.split_relayer)?;

        ctx.accounts.express_relay_metadata.bump = ctx.bumps.express_relay_metadata;
        ctx.accounts.express_relay_metadata.admin = *ctx.accounts.admin.key;
        ctx.accounts.express_relay_metadata.relayer_signer = *ctx.accounts.relayer_signer.key;
        ctx.accounts.express_relay_metadata.relayer_fee_receiver = *ctx.accounts.relayer_fee_receiver.key;
        ctx.accounts.express_relay_metadata.split_protocol_default = data.split_protocol_default;
        ctx.accounts.express_relay_metadata.split_relayer = data.split_relayer;

        Ok(())
    }

    pub fn set_relayer(ctx: Context<SetRelayer>, _data: SetRelayerArgs) -> Result<()> {
        ctx.accounts.express_relay_metadata.relayer_signer = *ctx.accounts.relayer_signer.key;
        ctx.accounts.express_relay_metadata.relayer_fee_receiver = *ctx.accounts.relayer_fee_receiver.key;

        Ok(())
    }

    pub fn set_splits(ctx: Context<SetSplits>, data: SetSplitsArgs) -> Result<()> {
        validate_fee_split(data.split_protocol_default)?;
        validate_fee_split(data.split_relayer)?;

        ctx.accounts.express_relay_metadata.split_protocol_default = data.split_protocol_default;
        ctx.accounts.express_relay_metadata.split_relayer = data.split_relayer;

        Ok(())
    }

    pub fn set_protocol_split(ctx: Context<SetProtocolSplit>, data: SetProtocolSplitArgs) -> Result<()> {
        validate_fee_split(data.split_protocol)?;

        ctx.accounts.protocol_config.split = data.split_protocol;

        Ok(())
    }

    pub fn permission(ctx: Context<Permission>, data: PermissionArgs) -> Result<()> {
        let relayer_signer = &ctx.accounts.relayer_signer;
        let permission = &ctx.accounts.permission;
        let sysvar_ixs = &ctx.accounts.sysvar_instructions;

        // check that current permissioning ix is first
        let index_permission = load_current_index_checked(sysvar_ixs)?;
        if index_permission != 0 {
            return err!(ExpressRelayError::PermissioningOutOfOrder)
        }

        // check that no intermediate instructions use relayer_signer
        let num_instructions = read_u16(&mut 0, &sysvar_ixs.data.borrow()).map_err(|_| ProgramError::InvalidInstructionData)?;
        for index in 1..num_instructions-1 {
            let ix = load_instruction_at_checked(index as usize, sysvar_ixs)?;
            if ix.accounts.iter().any(|acc| acc.pubkey == *relayer_signer.key) {
                return err!(ExpressRelayError::RelayerSignerUsedElsewhere)
            }
        }

        // check that last instruction is depermission, with matching permission pda
        let ix_depermission = load_instruction_at_checked((num_instructions-1) as usize, sysvar_ixs)?;
        let proper_depermissioning = (ix_depermission.program_id == *ctx.program_id) && (ix_depermission.accounts[1].pubkey == permission.key()) && (ix_depermission.data[0] == 5);
        if !proper_depermissioning {
            return err!(ExpressRelayError::PermissioningOutOfOrder)
        }

        let permission = &mut ctx.accounts.permission;
        permission.bump = ctx.bumps.permission;
        permission.balance = permission.to_account_info().lamports();
        permission.bid_amount = data.bid_amount;

        Ok(())
    }

    pub fn depermission(ctx: Context<Depermission>, _data: DepermissionArgs) -> Result<()> {
        let permission = &ctx.accounts.permission;
        let protocol_config = &ctx.accounts.protocol_config;
        let express_relay_metadata = &ctx.accounts.express_relay_metadata;
        let protocol = &ctx.accounts.protocol;
        let relayer_fee_receiver = &ctx.accounts.relayer_fee_receiver;

        if permission.to_account_info().lamports() < permission.balance + permission.bid_amount {
            return err!(ExpressRelayError::BidNotMet)
        }

        let split_protocol: u64;
        if protocol_config.to_account_info().data_len() > 0 {
            split_protocol = protocol_config.split;
        } else {
            split_protocol = express_relay_metadata.split_protocol_default;
        }

        let fee_protocol = permission.bid_amount * split_protocol / FEE_SPLIT_PRECISION;
        let fee_relayer = (permission.bid_amount - fee_protocol) * express_relay_metadata.split_relayer / FEE_SPLIT_PRECISION;

        transfer_lamports(&permission.to_account_info(), &protocol.to_account_info(), fee_protocol)?;
        transfer_lamports(&permission.to_account_info(), &relayer_fee_receiver.to_account_info(), fee_relayer)?;

        Ok(())
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
    /// CHECK: this is just a keypair for the admin
    pub admin: UncheckedAccount<'info>,
    /// CHECK: this is just a keypair for the relayer to sign with
    pub relayer_signer: UncheckedAccount<'info>,
    /// CHECK: this is just a public key for the relayer to receive fees at
    pub relayer_fee_receiver: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Eq, PartialEq, Clone, Copy, Debug)]
pub struct SetRelayerArgs {}

#[derive(Accounts)]
pub struct SetRelayer<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,
    #[account(mut, seeds = [SEED_METADATA], bump = express_relay_metadata.bump, has_one = admin, has_one = relayer_signer, has_one = relayer_fee_receiver)]
    pub express_relay_metadata: Account<'info, ExpressRelayMetadata>,
    /// CHECK: this is just a keypair for the relayer to sign with
    pub relayer_signer: UncheckedAccount<'info>,
    /// CHECK: this is just a public key for the relayer to receive fees at
    pub relayer_fee_receiver: UncheckedAccount<'info>,
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
    #[account(mut, seeds = [SEED_METADATA], bump = express_relay_metadata.bump, has_one = admin)]
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
    #[account(seeds = [SEED_METADATA], bump = express_relay_metadata.bump, has_one = admin)]
    pub express_relay_metadata: Account<'info, ExpressRelayMetadata>,
    /// CHECK: this is just the protocol fee receiver address
    pub protocol: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Eq, PartialEq, Clone, Debug)]
pub struct PermissionArgs {
    pub permission_id: Box<[u8]>,
    pub bid_id: [u8; 16],
    pub bid_amount: u64,
}

#[derive(Accounts)]
#[instruction(data: PermissionArgs)]
pub struct Permission<'info> {
    #[account(mut)]
    pub relayer_signer: Signer<'info>,
    #[account(init, payer = relayer_signer, space = RESERVE_PERMISSION, seeds = [SEED_PERMISSION, protocol.key().as_ref(), &data.permission_id], bump)]
    pub permission: Account<'info, PermissionMetadata>,
    /// CHECK: this is just the protocol fee receiver address
    pub protocol: UncheckedAccount<'info>,
    #[account(seeds = [SEED_METADATA], bump = express_relay_metadata.bump, has_one = relayer_signer)]
    pub express_relay_metadata: Account<'info, ExpressRelayMetadata>,
    pub system_program: Program<'info, System>,
    // TODO: https://github.com/solana-labs/solana/issues/22911
    /// CHECK: this is the sysvar instructions account
    #[account(address = tx_instructions::ID)]
    pub sysvar_instructions: UncheckedAccount<'info>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Eq, PartialEq, Clone, Debug)]
pub struct DepermissionArgs {
    pub permission_id: Box<[u8]>,
    pub bid_id: [u8; 16],
}

#[derive(Accounts)]
#[instruction(data: DepermissionArgs)]
pub struct Depermission<'info> {
    #[account(mut)]
    pub relayer_signer: Signer<'info>,
    #[account(mut, seeds = [SEED_PERMISSION, protocol.key().as_ref(), &data.permission_id], bump = permission.bump, close = relayer_signer)]
    pub permission: Account<'info, PermissionMetadata>,
    /// CHECK: this is just the protocol fee receiver address
    #[account(mut)]
    pub protocol: UncheckedAccount<'info>,
    /// CHECK: this is just a public key for the relayer to receive fees at
    #[account(mut)]
    pub relayer_fee_receiver: UncheckedAccount<'info>,
    #[account(seeds = [SEED_CONFIG_PROTOCOL, protocol.key().as_ref()], bump)]
    pub protocol_config: Account<'info, ConfigProtocol>,
    #[account(seeds = [SEED_METADATA], bump = express_relay_metadata.bump, has_one = relayer_signer, has_one = relayer_fee_receiver)]
    pub express_relay_metadata: Account<'info, ExpressRelayMetadata>,
    pub system_program: Program<'info, System>,
}
