use borsh::{BorshSerialize, BorshDeserialize};
use solana_program::{
    account_info::{AccountInfo, next_account_info}, entrypoint::ProgramResult, msg, program_error::ProgramError,
    borsh1::try_from_slice_unchecked,
    pubkey::Pubkey,
    serialize_utils::read_u16,
    system_program,
    system_instruction,
    sysvar::instructions::{id as sysvar_instructions_id, load_current_index_checked, load_instruction_at_checked},
    program::invoke_signed,
    sysvar::rent::Rent,
};

use crate::{
    instruction::{InitializeArgs, SetRelayerArgs, SetSplitsArgs, SetProtocolSplitArgs, PermissionArgs, DepermissionArgs, ExpressRelayInstruction, INDEX_DEPERMISSION},
    error::ExpressRelayError,
    utils::transfer_lamports,
    validation_utils::{assert_keys_equal, validate_fee_split},
    state::{ExpressRelayMetadata, PermissionMetadata, ConfigProtocol, SEED_METADATA, SEED_PERMISSION, SEED_CONFIG_PROTOCOL, RESERVE_EXPRESS_RELAY_METADATA, RESERVE_PERMISSION, RESERVE_EXPRESS_RELAY_CONFIG_PROTOCOL, FEE_SPLIT_PRECISION},
};

pub struct Processor;
impl Processor {
    pub fn process(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        instruction_data: &[u8],
    ) -> ProgramResult {
        let (tag, rest) = instruction_data
            .split_first()
            .ok_or(ProgramError::InvalidInstructionData)?;
        let instruction = ExpressRelayInstruction::unpack(tag)?;
        match instruction {
            ExpressRelayInstruction::Initialize => {
                let args = InitializeArgs::try_from_slice(rest)?;
                msg!("Instruction: initializing");
                Self::process_initialize(program_id, accounts, &args)
            }
            ExpressRelayInstruction::SetRelayer => {
                let args = SetRelayerArgs::try_from_slice(rest)?;
                msg!("Instruction: setting relayer");
                Self::process_set_relayer(program_id, accounts, &args)
            }
            ExpressRelayInstruction::SetSplits => {
                let args = SetSplitsArgs::try_from_slice(rest)?;
                msg!("Instruction: setting splits");
                Self::process_set_splits(program_id, accounts, &args)
            }
            ExpressRelayInstruction::SetProtocolSplit => {
                let args = SetProtocolSplitArgs::try_from_slice(rest)?;
                msg!("Instruction: setting protocol split");
                Self::process_set_protocol_split(program_id, accounts, &args)
            }
            ExpressRelayInstruction::Permission => {
                let args = PermissionArgs::try_from_slice(rest)?;
                msg!("Instruction: permissioning");
                Self::process_permission(program_id, accounts, &args)
            }
            ExpressRelayInstruction::Depermission => {
                let args = DepermissionArgs::try_from_slice(rest)?;
                msg!("Instruction: depermissioning");
                Self::process_depermission(program_id, accounts, &args)
            }
        }
    }

    fn process_initialize(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        data: &InitializeArgs,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let payer = next_account_info(account_info_iter)?;

        let metadata = next_account_info(account_info_iter)?;
        let (pda_metadata, bump_metadata) = Pubkey::find_program_address(&[SEED_METADATA], program_id);
        assert_keys_equal(pda_metadata, *metadata.key)?;

        let admin = next_account_info(account_info_iter)?;

        let relayer_signer = next_account_info(account_info_iter)?;

        let relayer_fee_receiver = next_account_info(account_info_iter)?;

        let system_program = next_account_info(account_info_iter)?;

        if metadata.data_len() != 0 {
            return Err(ExpressRelayError::AlreadyInitialized.into());
        }
        // create the express relay metadata account
        let required_lamports = Rent::default().minimum_balance(RESERVE_EXPRESS_RELAY_METADATA).max(1).saturating_sub(metadata.lamports());
        invoke_signed(
            &system_instruction::create_account(
                payer.key,
                metadata.key,
                required_lamports,
                RESERVE_EXPRESS_RELAY_METADATA as u64,
                program_id,
            ),
            &[
                payer.clone(),
                metadata.clone(),
                system_program.clone(),
            ],
            &[&[SEED_METADATA, &[bump_metadata]]],
        )?;

        validate_fee_split(data.split_protocol_default)?;
        validate_fee_split(data.split_relayer)?;

        let mut express_relay_data: ExpressRelayMetadata = try_from_slice_unchecked(&metadata.data.borrow_mut())?;
        express_relay_data.bump = bump_metadata;
        express_relay_data.admin = *admin.key;
        express_relay_data.relayer_signer = *relayer_signer.key;
        express_relay_data.relayer_fee_receiver = *relayer_fee_receiver.key;
        express_relay_data.split_protocol_default = data.split_protocol_default;
        express_relay_data.split_relayer = data.split_relayer;
        express_relay_data.serialize(&mut *metadata.data.borrow_mut())?;
        Ok(())
    }

    fn process_set_relayer(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        _args: &SetRelayerArgs,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let admin = next_account_info(account_info_iter)?;

        if !admin.is_signer {
            return Err(ProgramError::MissingRequiredSignature);
        }

        let metadata = next_account_info(account_info_iter)?;
        let mut metadata_data: ExpressRelayMetadata = try_from_slice_unchecked(&metadata.data.borrow_mut())?;
        let pda_metadata = Pubkey::create_program_address(&[SEED_METADATA, &[metadata_data.bump]], &program_id)?;
        assert_keys_equal(pda_metadata, *metadata.key)?;
        assert_keys_equal(metadata_data.admin, *admin.key)?;

        let relayer_signer = next_account_info(account_info_iter)?;

        let relayer_fee_receiver = next_account_info(account_info_iter)?;

        metadata_data.relayer_signer = *relayer_signer.key;
        metadata_data.relayer_fee_receiver = *relayer_fee_receiver.key;
        metadata_data.serialize(&mut *metadata.data.borrow_mut())?;
        Ok(())
    }

    fn process_set_splits(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        args: &SetSplitsArgs,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let admin = next_account_info(account_info_iter)?;

        if !admin.is_signer {
            return Err(ProgramError::MissingRequiredSignature);
        }

        let metadata = next_account_info(account_info_iter)?;
        let mut metadata_data: ExpressRelayMetadata = try_from_slice_unchecked(&metadata.data.borrow_mut())?;
        let pda_metadata = Pubkey::create_program_address(&[SEED_METADATA, &[metadata_data.bump]], program_id)?;
        assert_keys_equal(pda_metadata, *metadata.key)?;
        assert_keys_equal(metadata_data.admin, *admin.key)?;

        validate_fee_split(args.split_protocol_default)?;
        validate_fee_split(args.split_relayer)?;

        metadata_data.split_protocol_default = args.split_protocol_default;
        metadata_data.split_relayer = args.split_relayer;
        metadata_data.serialize(&mut *metadata.data.borrow_mut())?;
        Ok(())
    }

    fn process_set_protocol_split(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        args: &SetProtocolSplitArgs,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let admin = next_account_info(account_info_iter)?;

        if !admin.is_signer {
            return Err(ProgramError::MissingRequiredSignature);
        }

        let protocol_config = next_account_info(account_info_iter)?;

        let metadata = next_account_info(account_info_iter)?;
        let metadata_data: ExpressRelayMetadata = try_from_slice_unchecked(&metadata.data.borrow())?;
        let pda_metadata = Pubkey::create_program_address(&[SEED_METADATA, &[metadata_data.bump]], program_id)?;
        assert_keys_equal(pda_metadata, *metadata.key)?;
        assert_keys_equal(metadata_data.admin, *admin.key)?;

        let protocol = next_account_info(account_info_iter)?;

        let system_program = next_account_info(account_info_iter)?;
        assert_keys_equal(system_program::id(), *system_program.key)?;

        let mut protocol_data: ConfigProtocol;
        let pda_protocol_config: Pubkey;
        let bump_protocol_config: u8;
        if protocol_config.data_len() == 0 {
            (pda_protocol_config, bump_protocol_config) = Pubkey::find_program_address(&[SEED_CONFIG_PROTOCOL, &protocol.key.to_bytes()], program_id);

            let required_lamports = Rent::default().minimum_balance(RESERVE_EXPRESS_RELAY_CONFIG_PROTOCOL).max(1).saturating_sub(protocol_config.lamports());
            invoke_signed(
                &system_instruction::create_account(
                    admin.key,
                    protocol_config.key,
                    required_lamports,
                    RESERVE_EXPRESS_RELAY_CONFIG_PROTOCOL as u64,
                    program_id,
                ),
                &[
                    admin.clone(),
                    protocol_config.clone(),
                    system_program.clone(),
                ],
                &[&[SEED_CONFIG_PROTOCOL, &protocol.key.to_bytes(), &[bump_protocol_config]]],
            )?;

            protocol_data = try_from_slice_unchecked(&protocol_config.data.borrow_mut())?;

            protocol_data.bump = bump_protocol_config;
        } else {
            protocol_data = try_from_slice_unchecked(&protocol_config.data.borrow_mut())?;
            pda_protocol_config = Pubkey::create_program_address(&[SEED_CONFIG_PROTOCOL, &protocol.key.to_bytes(), &[protocol_data.bump]], program_id)?;
        }

        assert_keys_equal(pda_protocol_config, *protocol_config.key)?;

        validate_fee_split(args.split_protocol)?;

        protocol_data.split = args.split_protocol;
        protocol_data.serialize(&mut *protocol_config.data.borrow_mut())?;
        Ok(())
    }

    fn process_permission(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        args: &PermissionArgs,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let relayer_signer = next_account_info(account_info_iter)?;

        if !relayer_signer.is_signer {
            return Err(ProgramError::MissingRequiredSignature);
        }

        let permission = next_account_info(account_info_iter)?;

        let protocol = next_account_info(account_info_iter)?;
        let (pda_permission, bump_permission) = Pubkey::find_program_address(&[SEED_PERMISSION, &protocol.key.to_bytes(), &args.permission_id], program_id);
        assert_keys_equal(pda_permission, *permission.key)?;

        let metadata = next_account_info(account_info_iter)?;
        let metadata_data: ExpressRelayMetadata = try_from_slice_unchecked(&metadata.data.borrow())?;
        let pda_metadata = Pubkey::create_program_address(&[SEED_METADATA, &[metadata_data.bump]], program_id)?;
        assert_keys_equal(pda_metadata, *metadata.key)?;
        assert_keys_equal(metadata_data.relayer_signer, *relayer_signer.key)?;

        let system_program = next_account_info(account_info_iter)?;
        assert_keys_equal(system_program::id(), *system_program.key)?;

        let sysvar_ixs = next_account_info(account_info_iter)?;
        assert_keys_equal(sysvar_instructions_id(), *sysvar_ixs.key)?;

        // check that current permissioning ix is first
        let index_permission = load_current_index_checked(sysvar_ixs)?;
        if index_permission != 0 {
            return Err(ExpressRelayError::PermissioningOutOfOrder.into());
        }

        // check that no intermediate instructions use relayer_signer
        let num_instructions = read_u16(&mut 0, &sysvar_ixs.data.borrow()).map_err(|_| ProgramError::InvalidInstructionData)?;
        for index in 1..num_instructions-1 {
            let ix = load_instruction_at_checked(index as usize, sysvar_ixs)?;
            if ix.accounts.iter().any(|acc| acc.pubkey == *relayer_signer.key) {
                return Err(ExpressRelayError::RelayerSignerUsedElsewhere.into());
            }
        }

        // check that last instruction is depermission, with matching permission pda
        let ix_depermission = load_instruction_at_checked((num_instructions-1) as usize, sysvar_ixs)?;
        let proper_depermissioning = (ix_depermission.program_id == *program_id) && (ix_depermission.accounts[1].pubkey == *permission.key) && (ix_depermission.data[0] == INDEX_DEPERMISSION);
        if !proper_depermissioning {
            return Err(ExpressRelayError::PermissioningOutOfOrder.into());
        }

        if permission.data_len() != 0 {
            return Err(ExpressRelayError::PermissionAlreadyToggled.into());
        }

        let required_lamports = Rent::default().minimum_balance(RESERVE_PERMISSION).max(1).saturating_sub(permission.lamports());
        invoke_signed(
            &system_instruction::create_account(
                relayer_signer.key,
                permission.key,
                required_lamports,
                RESERVE_PERMISSION as u64,
                program_id,
            ),
            &[
                relayer_signer.clone(),
                permission.clone(),
                system_program.clone(),
            ],
            &[&[SEED_PERMISSION, &protocol.key.to_bytes(), &args.permission_id, &[bump_permission]]],
        )?;

        let mut permission_data: PermissionMetadata = try_from_slice_unchecked(&permission.data.borrow_mut())?;
        permission_data.bump = bump_permission;
        permission_data.balance = permission.lamports();
        permission_data.bid_amount = args.bid_amount;
        permission_data.serialize(&mut *permission.data.borrow_mut())?;
        Ok(())
    }

    fn process_depermission(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        args: &DepermissionArgs,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let relayer_signer = next_account_info(account_info_iter)?;

        if !relayer_signer.is_signer {
            return Err(ProgramError::MissingRequiredSignature);
        }

        let permission = next_account_info(account_info_iter)?;
        let permission_data: PermissionMetadata = try_from_slice_unchecked(&permission.data.borrow())?;

        let protocol = next_account_info(account_info_iter)?;
        let pda_permission = Pubkey::create_program_address(&[SEED_PERMISSION, &protocol.key.to_bytes(), &args.permission_id, &[permission_data.bump]], program_id)?;
        assert_keys_equal(pda_permission, *permission.key)?;

        let relayer_fee_receiver = next_account_info(account_info_iter)?;

        let protocol_config = next_account_info(account_info_iter)?;

        let metadata = next_account_info(account_info_iter)?;
        let metadata_data: ExpressRelayMetadata = try_from_slice_unchecked(&metadata.data.borrow())?;
        let pda_metadata = Pubkey::create_program_address(&[SEED_METADATA, &[metadata_data.bump]], program_id)?;
        assert_keys_equal(pda_metadata, *metadata.key)?;
        assert_keys_equal(metadata_data.relayer_signer, *relayer_signer.key)?;
        assert_keys_equal(metadata_data.relayer_fee_receiver, *relayer_fee_receiver.key)?;

        let mut fee_split_protocol = metadata_data.split_protocol_default;
        if protocol_config.data_len() == 0 {
            let (pda_protocol_config, _) = Pubkey::find_program_address(&[SEED_CONFIG_PROTOCOL, &protocol.key.to_bytes()], program_id);
            assert_keys_equal(pda_protocol_config, *protocol_config.key)?;
        } else {
            let protocol_data: ConfigProtocol = try_from_slice_unchecked(&protocol_config.data.borrow())?;
            let pda_protocol_config = Pubkey::create_program_address(&[SEED_CONFIG_PROTOCOL, &protocol.key.to_bytes(), &[protocol_data.bump]], program_id)?;
            assert_keys_equal(pda_protocol_config, *protocol_config.key)?;
            fee_split_protocol = protocol_data.split;
        }

        let system_program = next_account_info(account_info_iter)?;
        assert_keys_equal(system_program::id(), *system_program.key)?;

        let balance_pre = permission_data.balance;
        let bid_amount = permission_data.bid_amount;

        if permission.lamports() < balance_pre + bid_amount {
            return Err(ExpressRelayError::BidNotMet.into());
        }

        let fee_protocol = bid_amount * fee_split_protocol / FEE_SPLIT_PRECISION;
        let fee_relayer = (bid_amount - fee_protocol) * metadata_data.split_relayer / FEE_SPLIT_PRECISION;

        transfer_lamports(permission, protocol, fee_protocol)?;
        transfer_lamports(permission, relayer_fee_receiver, fee_relayer)?;

        // close permission account
        permission.data.borrow_mut().fill(0);
        transfer_lamports(permission, relayer_signer, permission.lamports())?;

        Ok(())
    }
}

// TODO: use anchor
