pub mod error;
pub mod state;
pub mod utils;

use anchor_lang::{prelude::*, system_program::System};
use anchor_lang::solana_program::sysvar::instructions as tx_instructions;
use solana_program::{serialize_utils::read_u16, sysvar::instructions::{load_current_index_checked, load_instruction_at_checked}};
use anchor_syn::codegen::program::common::sighash;
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
        // TODO: do we need to do a checked_sub/saturating_sub here?
        let last_ix_index = num_instructions - 1;
        for index in 1..last_ix_index {
            let ix = load_instruction_at_checked(index as usize, sysvar_ixs)?;
            if ix.accounts.iter().any(|acc| acc.pubkey == *relayer_signer.key) {
                return err!(ExpressRelayError::RelayerSignerUsedElsewhere)
            }
        }

        // check that last instruction is depermission, with matching permission pda
        let ix_depermission = load_instruction_at_checked(last_ix_index as usize, sysvar_ixs)?;
        // anchor discriminator comes from the hash of "{namespace}:{name}" https://github.com/coral-xyz/anchor/blob/2a07d841c65d6f303aa9c2b0c68a6e69c4739aab/lang/syn/src/codegen/program/common.rs#L9-L23
        let program_equal = ix_depermission.program_id == *ctx.program_id;
        // TODO: can we make this matching permission accounts check more robust (e.g. using account names in addition, to not rely on ordering alone)?
        let matching_permission_accounts = ix_depermission.accounts[1].pubkey == permission.key();
        let expected_discriminator = sighash("global", "depermission");
        let matching_discriminator = ix_depermission.data[0..8] == expected_discriminator;
        let proper_depermissioning = program_equal && matching_permission_accounts && matching_discriminator;
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
        let protocol_fee_receiver = &ctx.accounts.protocol_fee_receiver;
        let relayer_fee_receiver = &ctx.accounts.relayer_fee_receiver;

        if permission.to_account_info().lamports() < permission.balance.saturating_add(permission.bid_amount) {
            return err!(ExpressRelayError::BidNotMet)
        }

        let split_protocol: u64;
        let protocol_config_account_info = protocol_config.to_account_info();
        if protocol_config_account_info.data_len() > 0 {
            let account_data = &mut &**protocol_config_account_info.try_borrow_data()?;
            let protocol_config_data = ConfigProtocol::try_deserialize(account_data)?;
            split_protocol = protocol_config_data.split;
        } else {
            split_protocol = express_relay_metadata.split_protocol_default;
        }

        let fee_protocol = permission.bid_amount * split_protocol / FEE_SPLIT_PRECISION;
        if fee_protocol > permission.bid_amount {
            return err!(ExpressRelayError::FeesTooHigh);
        }
        let fee_relayer = permission.bid_amount.saturating_sub(fee_protocol) * express_relay_metadata.split_relayer / FEE_SPLIT_PRECISION;
        if fee_relayer.checked_add(fee_protocol).unwrap() > permission.bid_amount {
            return err!(ExpressRelayError::FeesTooHigh);
        }

        transfer_lamports(&permission.to_account_info(), &protocol_fee_receiver.to_account_info(), fee_protocol)?;
        transfer_lamports(&permission.to_account_info(), &relayer_fee_receiver.to_account_info(), fee_relayer)?;
        // send the remaining balance from the bid to the express relay metadata account
        transfer_lamports(&permission.to_account_info(), &express_relay_metadata.to_account_info(), permission.bid_amount.saturating_sub(fee_protocol).saturating_sub(fee_relayer))?;

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
    /// CHECK: this is just the PK for the admin to sign from
    pub admin: UncheckedAccount<'info>,
    /// CHECK: this is just the PK for the relayer to sign from
    pub relayer_signer: UncheckedAccount<'info>,
    /// CHECK: this is just a PK for the relayer to receive fees at
    pub relayer_fee_receiver: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Eq, PartialEq, Clone, Copy, Debug)]
pub struct SetRelayerArgs {}

#[derive(Accounts)]
pub struct SetRelayer<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,
    #[account(mut, seeds = [SEED_METADATA], bump = express_relay_metadata.bump, has_one = admin)]
    pub express_relay_metadata: Account<'info, ExpressRelayMetadata>,
    /// CHECK: this is just the PK for the relayer to sign from
    pub relayer_signer: UncheckedAccount<'info>,
    /// CHECK: this is just a PK for the relayer to receive fees at
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
    /// CHECK: this is just the protocol fee receiver PK
    pub protocol: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Eq, PartialEq, Clone, Copy, Debug)]
pub struct PermissionArgs {
    pub permission_id: [u8; 32],
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
    /// CHECK: this is just the protocol fee receiver PK
    pub protocol: UncheckedAccount<'info>,
    #[account(seeds = [SEED_METADATA], bump = express_relay_metadata.bump, has_one = relayer_signer)]
    pub express_relay_metadata: Account<'info, ExpressRelayMetadata>,
    pub system_program: Program<'info, System>,
    // TODO: https://github.com/solana-labs/solana/issues/22911
    /// CHECK: this is the sysvar instructions account
    #[account(address = tx_instructions::ID)]
    pub sysvar_instructions: UncheckedAccount<'info>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Eq, PartialEq, Clone, Copy, Debug)]
pub struct DepermissionArgs {
    pub permission_id: [u8; 32],
    pub bid_id: [u8; 16],
}

#[derive(Accounts)]
#[instruction(data: DepermissionArgs)]
pub struct Depermission<'info> {
    #[account(mut)]
    pub relayer_signer: Signer<'info>,
    // TODO: upon close, should send funds to the program as opposed to the relayer signer--o/w relayer will get all "fat-fingered" fees
    #[account(mut, seeds = [SEED_PERMISSION, protocol.key().as_ref(), &data.permission_id], bump, close = relayer_signer)]
    pub permission: Account<'info, PermissionMetadata>,
    /// CHECK: this is just the protocol program address
    pub protocol: UncheckedAccount<'info>,
    /// CHECK: don't care what this PDA looks like
    #[account(
        mut,
        seeds = [SEED_EXPRESS_RELAY_FEES],
        seeds::program = protocol.key(),
        bump
    )]
    pub protocol_fee_receiver: UncheckedAccount<'info>,
    /// CHECK: this is just a PK for the relayer to receive fees at
    #[account(mut)]
    pub relayer_fee_receiver: UncheckedAccount<'info>,
    /// CHECK: this cannot be checked against ConfigProtocol bc it may not be initialized bc anchor :(
    #[account(seeds = [SEED_CONFIG_PROTOCOL, protocol.key().as_ref()], bump)]
    pub protocol_config: UncheckedAccount<'info>,
    #[account(mut, seeds = [SEED_METADATA], bump = express_relay_metadata.bump, has_one = relayer_signer, has_one = relayer_fee_receiver)]
    pub express_relay_metadata: Account<'info, ExpressRelayMetadata>,
    pub system_program: Program<'info, System>,
}
