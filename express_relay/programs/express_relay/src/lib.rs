pub mod error;
pub mod state;
pub mod utils;

use anchor_lang::{prelude::*, system_program::System};
use anchor_lang::solana_program::sysvar::instructions as tx_instructions;
use solana_program::{hash, clock::Clock, serialize_utils::read_u16, sysvar::instructions::{load_current_index_checked, load_instruction_at_checked}};
use anchor_syn::codegen::program::common::sighash;
use anchor_spl::token::{self, TokenAccount, Token, Mint, Transfer as SplTransfer, CloseAccount};
use crate::{
    error::ExpressRelayError,
    state::*,
    utils::*,
};
use opportunity_adapter::ID as OPPORTUNITY_ADAPTER_PROGRAM_ID;
use core::time;
use std::str::FromStr;

declare_id!("AJ9QckBqWJdz5RAxpMi2P83q6R7y5xZ2yFxCAYr3bg3N");

#[inline(never)]
pub fn handle_wsol_transfer<'info>(
    wsol_ta_user: &Account<'info, TokenAccount>,
    wsol_ta_express_relay: &Account<'info, TokenAccount>,
    express_relay_authority: &AccountLoader<'info, Authority>,
    token_program: &Program<'info, Token>,
    bump_express_relay_authority: u8,
    permission: &AccountLoader<'info, PermissionMetadata>,
    wsol_mint: &Account<'info, Mint>,
    bump_wsol_ta_express_relay: u8,
) -> Result<()> {
    msg!("GOT TO HANDLE WSOL 1");
    let permission_data = permission.load()?;
    msg!("GOT TO HANDLE WSOL 2");
    let bid_amount = permission_data.bid_amount;
    drop(permission_data);
    msg!("bid amount {:p}", &bid_amount);

    msg!("GOT TO HANDLE WSOL 3");
    // wrapped sol transfer
    msg!("wsol_ta_user {:p}", wsol_ta_user);
    msg!("wsol_ta_user cloned {:p}", &(wsol_ta_user.to_account_info().clone()));
    msg!("wsol_ta_express_relay {:p}", wsol_ta_express_relay);
    msg!("wsol_ta_express_relay cloned {:p}", &(wsol_ta_express_relay.to_account_info().clone()));
    msg!("wsol_ta_express_relay acc info {:p}", &(wsol_ta_express_relay.to_account_info()));
    msg!("express_relay_authority {:p}", express_relay_authority);
    msg!("express_relay_authority cloned {:p}", &(express_relay_authority.to_account_info().clone()));
    let cpi_accounts_transfer = SplTransfer {
        from: wsol_ta_user.to_account_info().clone(),
        to: wsol_ta_express_relay.to_account_info().clone(),
        authority: express_relay_authority.to_account_info().clone(),
    };
    msg!("GOT TO HANDLE WSOL 4");
    let cpi_program_transfer = token_program.to_account_info();
    token::transfer(
        CpiContext::new_with_signer(
            cpi_program_transfer,
            cpi_accounts_transfer,
            &[
                &[
                    SEED_AUTHORITY,
                    &[bump_express_relay_authority]
                ]
            ]
        ),
        bid_amount
    )?;
    msg!("GOT TO HANDLE WSOL 5");
    // close wsol_ta_express_relay to get the SOL
    let cpi_accounts_close = CloseAccount {
        account: wsol_ta_express_relay.to_account_info().clone(),
        destination: permission.to_account_info().clone(),
        authority: wsol_ta_express_relay.to_account_info().clone(),
    };
    let cpi_program_close = token_program.to_account_info();
    token::close_account(
        CpiContext::new_with_signer(
            cpi_program_close,
            cpi_accounts_close,
            &[
                &[
                    b"ata",
                    wsol_mint.key().as_ref(),
                    &[bump_wsol_ta_express_relay]
                ]
            ]
        )
    )?;

    Ok(())
}

#[inline(never)]
pub fn validate_signature(
    sysvar_ixs: &UncheckedAccount,
    bid_amount: u64,
    data: DepermissionArgs,
    protocol_key: Pubkey,
    user_key: Pubkey,
) -> Result<()> {
    let permission_id = data.permission_id;
    let valid_until = data.valid_until;

    let timestamp = Clock::get()?.unix_timestamp as u64;
    msg!("DATA RN {:?}", data);
    if timestamp > valid_until {
        return err!(ExpressRelayError::SignatureExpired)
    }

    msg!("DATA {:p}", &data);
    msg!("TIMESTAMP {:p}", &timestamp);

    msg!("starting to load current index");
    let index_depermission = load_current_index_checked(sysvar_ixs)?;
    msg!("index deperm {:p}", &index_depermission);
    let ix = load_instruction_at_checked((index_depermission-1) as usize, sysvar_ixs)?;
    msg!("made ix");

    let mut msg_vec = [0; 32+32+32+8+8];
    msg!("instantiated msg_vec");
    msg_vec[..32].copy_from_slice(&protocol_key.to_bytes());
    msg_vec[32..64].copy_from_slice(&permission_id);
    msg_vec[64..96].copy_from_slice(&user_key.to_bytes());
    msg_vec[96..104].copy_from_slice(&bid_amount.to_le_bytes());
    msg_vec[104..112].copy_from_slice(&valid_until.to_le_bytes());
    msg!("copied to msg_vec");
    // TODO: uncomment and fix this
    let msg: &[u8] = &msg_vec;
    msg!("msg {:?}", msg);
    let digest = hash::hash(msg);
    msg!("protocol_key {:?}", protocol_key.to_bytes());
    msg!("permission_id {:?}", permission_id);
    msg!("user_key {:?}", user_key.to_bytes());
    msg!("bid_amount {:?}", bid_amount.to_le_bytes());
    msg!("valid_until {:?}", valid_until.to_le_bytes());

    msg!("digest {:?}", digest.as_ref());
    msg!("hashed msg");
    verify_ed25519_ix(&ix, &user_key.to_bytes(), digest.as_ref(), &data.signature)?;

    msg!("Finished validation of signature");

    Ok(())
}

#[program]
pub mod express_relay {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>, data: InitializeArgs) -> Result<()> {
        validate_fee_split(data.split_protocol_default)?;
        validate_fee_split(data.split_relayer)?;

        let express_relay_metadata_data = &mut ctx.accounts.express_relay_metadata.load_init()?;
        // ctx.accounts.express_relay_metadata.bump = ctx.bumps.express_relay_metadata;
        express_relay_metadata_data.admin = *ctx.accounts.admin.key;
        express_relay_metadata_data.relayer_signer = *ctx.accounts.relayer_signer.key;
        express_relay_metadata_data.relayer_fee_receiver = *ctx.accounts.relayer_fee_receiver.key;
        express_relay_metadata_data.split_protocol_default = data.split_protocol_default;
        express_relay_metadata_data.split_relayer = data.split_relayer;

        Ok(())
    }

    pub fn set_relayer(ctx: Context<SetRelayer>, _data: SetRelayerArgs) -> Result<()> {
        let express_relay_metadata_data = &mut ctx.accounts.express_relay_metadata.load_mut()?;

        express_relay_metadata_data.relayer_signer = *ctx.accounts.relayer_signer.key;
        express_relay_metadata_data.relayer_fee_receiver = *ctx.accounts.relayer_fee_receiver.key;

        Ok(())
    }

    pub fn set_splits(ctx: Context<SetSplits>, data: SetSplitsArgs) -> Result<()> {
        validate_fee_split(data.split_protocol_default)?;
        validate_fee_split(data.split_relayer)?;

        let express_relay_metadata_data = &mut ctx.accounts.express_relay_metadata.load_mut()?;

        express_relay_metadata_data.split_protocol_default = data.split_protocol_default;
        express_relay_metadata_data.split_relayer = data.split_relayer;

        Ok(())
    }

    pub fn set_protocol_split(ctx: Context<SetProtocolSplit>, data: SetProtocolSplitArgs) -> Result<()> {
        validate_fee_split(data.split_protocol)?;

        ctx.accounts.protocol_config.split = data.split_protocol;

        Ok(())
    }

    pub fn permission(ctx: Context<Permission>, data: Box<PermissionArgs>) -> Result<()> {
        let relayer_signer = &ctx.accounts.relayer_signer;
        let permission = &ctx.accounts.permission;
        let sysvar_ixs = &ctx.accounts.sysvar_instructions;

        // check that current permissioning ix is first (TODO: this may be only relevant if we are checking no relayer_signer in future ixs)
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
            // TODO: we are going to have to figure out security here, preventing relayer_signer from signing for bad ixs
            // // only opportunity adapter allowed to use relayer signer as an account
            // if ix.program_id != OPPORTUNITY_ADAPTER_PROGRAM_ID {
            //     if ix.accounts.iter().any(|acc| acc.pubkey == *relayer_signer.key) {
            //         return err!(ExpressRelayError::RelayerSignerUsedElsewhere)
            //     }
            // }
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

        let permission_data = &mut permission.load_init()?;
        // permission.bump = ctx.bumps.permission;
        permission_data.balance = permission.to_account_info().lamports();
        permission_data.bid_amount = data.bid_amount;
        msg!("permission_data {:p}", permission_data);

        Ok(())
    }

    pub fn depermission(ctx: Context<Depermission>, data: DepermissionArgs) -> Result<()> {
        let check_space = [0u8; 1000];
        msg!("check_space {:p}", &check_space);

        let relayer_signer = &ctx.accounts.relayer_signer;
        msg!("relayer signer {:p}", relayer_signer);
        let permission = &ctx.accounts.permission;
        msg!("permission {:p}", permission);
        let protocol_config = &ctx.accounts.protocol_config;
        msg!("protocol_config {:p}", protocol_config);
        let express_relay_metadata = &ctx.accounts.express_relay_metadata;
        msg!("express_relay_metadata {:p}", express_relay_metadata);
        let protocol_fee_receiver = &ctx.accounts.protocol_fee_receiver;
        msg!("protocol_fee_receiver {:p}", protocol_fee_receiver);
        let relayer_fee_receiver = &ctx.accounts.relayer_fee_receiver;
        msg!("relayer_fee_receiver {:p}", relayer_fee_receiver);

        let wsol_mint = &ctx.accounts.wsol_mint;
        msg!("wsol_mint {:p}", wsol_mint);
        let wsol_ta_user = &ctx.accounts.wsol_ta_user;
        msg!("wsol_ta_user {:p}", wsol_ta_user);
        let wsol_ta_express_relay = &ctx.accounts.wsol_ta_express_relay;
        msg!("wsol_ta_express_relay {:p}", wsol_ta_express_relay);
        let express_relay_authority = &ctx.accounts.express_relay_authority;
        msg!("express_relay_authority {:p}", express_relay_authority);
        let token_program = &ctx.accounts.token_program;
        msg!("token_program {:p}", token_program);
        let sysvar_ixs = &ctx.accounts.sysvar_instructions;
        msg!("sysvar_ixs {:p}", sysvar_ixs);

        let j: u8 = 0;
        msg!("j! {:p}", &j);
        let j2: u8 = 1;
        msg!("j2! {:p}", &j2);

        msg!("ix data {:p}", &data);
        msg!("ctx {:p}", &ctx);

        let permission_data = permission.load()?;
        msg!("permission data {:p}", &permission_data);
        let bid_amount = permission_data.bid_amount;
        drop(permission_data);

        let express_relay_metadata_data = express_relay_metadata.load()?;
        let split_protocol_default = express_relay_metadata_data.split_protocol_default;
        let split_relayer = express_relay_metadata_data.split_relayer;
        drop(express_relay_metadata_data);

        // signature verification
        validate_signature(sysvar_ixs, bid_amount, data, ctx.accounts.protocol.key(), ctx.accounts.user.key())?;

        let rent_owed_relayer_signer = wsol_ta_express_relay.to_account_info().lamports();

        msg!("CHECK 1");
        handle_wsol_transfer(
            wsol_ta_user,
            wsol_ta_express_relay,
            express_relay_authority,
            token_program,
            ctx.bumps.express_relay_authority,
            permission,
            wsol_mint,
            ctx.bumps.wsol_ta_express_relay
        )?;

        // if permission.to_account_info().lamports() < permission.balance.saturating_add(permission.bid_amount) {
        //     return err!(ExpressRelayError::BidNotMet)
        // }

        msg!("CHECK 2");
        let split_protocol: u64;
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
            return err!(ExpressRelayError::FeesTooHigh);
        }
        let fee_relayer = bid_amount.saturating_sub(fee_protocol) * split_relayer / FEE_SPLIT_PRECISION;
        if fee_relayer.checked_add(fee_protocol).unwrap() > bid_amount {
            return err!(ExpressRelayError::FeesTooHigh);
        }
        msg!("CHECK 3");

        transfer_lamports(&permission.to_account_info(), &relayer_signer.to_account_info(), rent_owed_relayer_signer)?;
        transfer_lamports(&permission.to_account_info(), &protocol_fee_receiver.to_account_info(), fee_protocol)?;
        transfer_lamports(&permission.to_account_info(), &relayer_fee_receiver.to_account_info(), fee_relayer)?;
        // send the remaining balance from the bid to the express relay metadata account
        transfer_lamports(&permission.to_account_info(), &express_relay_metadata.to_account_info(), bid_amount.saturating_sub(fee_protocol).saturating_sub(fee_relayer))?;

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
    pub express_relay_metadata: AccountLoader<'info, ExpressRelayMetadata>,
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
    #[account(mut, seeds = [SEED_METADATA], bump, has_one = admin)]
    pub express_relay_metadata: AccountLoader<'info, ExpressRelayMetadata>,
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
    #[account(mut, seeds = [SEED_METADATA], bump, has_one = admin)]
    pub express_relay_metadata: AccountLoader<'info, ExpressRelayMetadata>,
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
    pub express_relay_metadata: AccountLoader<'info, ExpressRelayMetadata>,
    /// CHECK: this is just the protocol fee receiver PK
    pub protocol: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Eq, PartialEq, Clone, Copy, Debug)]
pub struct PermissionArgs {
    pub permission_id: [u8; 32],
    // TODO: maybe add bid_id back? depending on size constraints
    // pub bid_id: [u8; 16],
    pub bid_amount: u64,
}

#[derive(Accounts)]
#[instruction(data: PermissionArgs)]
pub struct Permission<'info> {
    #[account(mut)]
    pub relayer_signer: Signer<'info>,
    #[account(
        init,
        payer = relayer_signer,
        space = RESERVE_PERMISSION,
        seeds = [SEED_PERMISSION, protocol.key().as_ref(), &data.permission_id],
        bump
    )]
    pub permission: AccountLoader<'info, PermissionMetadata>,
    /// CHECK: this is just the protocol fee receiver PK
    pub protocol: UncheckedAccount<'info>,
    #[account(seeds = [SEED_METADATA], bump, has_one = relayer_signer)]
    pub express_relay_metadata: AccountLoader<'info, ExpressRelayMetadata>,
    pub system_program: Program<'info, System>,
    // TODO: https://github.com/solana-labs/solana/issues/22911
    /// CHECK: this is the sysvar instructions account
    #[account(address = tx_instructions::ID)]
    pub sysvar_instructions: UncheckedAccount<'info>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Eq, PartialEq, Clone, Copy, Debug)]
pub struct DepermissionArgs {
    pub permission_id: [u8; 32],
    pub signature: [u8; 64],
    pub valid_until: u64,
    // TODO: protect against replay attacks
}

#[derive(Accounts)]
// #[instruction(data: DepermissionArgs)]
pub struct Depermission<'info> {
    #[account(mut)]
    pub relayer_signer: Signer<'info>,
    // TODO: upon close, should send funds to the program as opposed to the relayer signer--o/w relayer will get all "fat-fingered" fees
    // TODO: need to do the pda validation
    // seeds = [SEED_PERMISSION, protocol.key().as_ref(), &data.permission_id],
    // bump,
    #[account(
        mut,
        close = relayer_signer)]
    pub permission: AccountLoader<'info, PermissionMetadata>,
    /// CHECK: this is just the user account
    pub user: UncheckedAccount<'info>,
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
    #[account(mut, seeds = [SEED_METADATA], bump, has_one = relayer_signer, has_one = relayer_fee_receiver)]
    pub express_relay_metadata: AccountLoader<'info, ExpressRelayMetadata>,
    #[account(constraint = wsol_mint.key() == Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap())]
    pub wsol_mint: Box<Account<'info, Mint>>,
    #[account(mut, token::mint = wsol_mint, token::authority = user)]
    pub wsol_ta_user: Box<Account<'info, TokenAccount>>,
    #[account(
        init,
        payer = relayer_signer,
        seeds = [b"ata", wsol_mint.key().as_ref()],
        bump,
        token::mint = wsol_mint,
        token::authority = wsol_ta_express_relay
    )]
    pub wsol_ta_express_relay: Box<Account<'info, TokenAccount>>,
    #[account(init_if_needed, payer = relayer_signer, space = 8+RESERVE_AUTHORITY, seeds = [SEED_AUTHORITY], bump)]
    pub express_relay_authority: AccountLoader<'info, Authority>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    /// CHECK: this is the sysvar instructions account
    #[account(address = tx_instructions::ID)]
    pub sysvar_instructions: UncheckedAccount<'info>,
}
