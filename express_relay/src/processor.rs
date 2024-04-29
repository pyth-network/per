use borsh::{BorshSerialize, BorshDeserialize};
use solana_program::{
    account_info::{AccountInfo, next_account_info}, entrypoint::ProgramResult, msg, program_error::ProgramError,
    borsh1::try_from_slice_unchecked,
    pubkey::Pubkey,
    system_program,
    system_instruction,
    program::invoke_signed,
    sysvar::rent::Rent,
};

use crate::{
    instruction::{InitializeArgs, SetRelayerArgs, SetSplitsArgs, PermissionArgs, DepermissionArgs, ExpressRelayInstruction},
    error::ExpressRelayError,
    validation_utils::{assert_keys_equal, validate_fee_splits},
    state::{ExpressRelayMetadata, PermissionMetadata, SEED_METADATA, SEED_PERMISSION, RESERVE_EXPRESS_RELAY_METADATA, RESERVE_PERMISSION},
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

        validate_fee_splits(data.split_protocol, data.split_relayer, data.split_precision)?;

        let mut express_relay_data: ExpressRelayMetadata = try_from_slice_unchecked(&metadata.data.borrow_mut())?;
        express_relay_data.bump = bump_metadata;
        express_relay_data.admin = *admin.key;
        express_relay_data.relayer_signer = *relayer_signer.key;
        express_relay_data.relayer_fee_receiver = *relayer_fee_receiver.key;
        express_relay_data.split_protocol = data.split_protocol;
        express_relay_data.split_relayer = data.split_relayer;
        express_relay_data.split_precision = data.split_precision;
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

        validate_fee_splits(args.split_protocol, args.split_relayer, args.split_precision)?;

        metadata_data.split_protocol = args.split_protocol;
        metadata_data.split_relayer = args.split_relayer;
        metadata_data.split_precision = args.split_precision;
        metadata_data.serialize(&mut *metadata.data.borrow_mut())?;
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

        let (pda_permission, bump_permission) = Pubkey::find_program_address(&[SEED_PERMISSION, &args.permission_key], program_id);
        assert_keys_equal(pda_permission, *permission.key)?;

        let metadata = next_account_info(account_info_iter)?;
        // can below be borrow instead of borrow_mut?
        let metadata_data: ExpressRelayMetadata = try_from_slice_unchecked(&metadata.data.borrow_mut())?;
        let pda_metadata = Pubkey::create_program_address(&[SEED_METADATA, &[metadata_data.bump]], program_id)?;
        assert_keys_equal(pda_metadata, *metadata.key)?;
        assert_keys_equal(metadata_data.relayer_signer, *relayer_signer.key)?;

        let system_program = next_account_info(account_info_iter)?;
        assert_keys_equal(system_program::id(), *system_program.key)?;

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
            &[&[SEED_PERMISSION, &args.permission_key, &[bump_permission]]],
        )?;

        let mut permission_data: PermissionMetadata = try_from_slice_unchecked(&permission.data.borrow_mut())?;
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
        let permission_data: PermissionMetadata = try_from_slice_unchecked(&permission.data.borrow_mut())?;
        let pda_permission = Pubkey::create_program_address(&[SEED_PERMISSION, &args.permission_key, &[permission_data.bump]], program_id)?;
        assert_keys_equal(pda_permission, *permission.key)?;

        let protocol = next_account_info(account_info_iter)?;
        // TODO: validate protocol given the permission key

        let relayer_fee_receiver = next_account_info(account_info_iter)?;

        let metadata = next_account_info(account_info_iter)?;
        let metadata_data: ExpressRelayMetadata = try_from_slice_unchecked(&metadata.data.borrow_mut())?;
        let pda_metadata = Pubkey::create_program_address(&[SEED_METADATA, &[metadata_data.bump]], program_id)?;
        assert_keys_equal(pda_metadata, *metadata.key)?;
        assert_keys_equal(metadata_data.relayer_signer, *relayer_signer.key)?;
        assert_keys_equal(metadata_data.relayer_fee_receiver, *relayer_fee_receiver.key)?;

        let system_program = next_account_info(account_info_iter)?;
        assert_keys_equal(system_program::id(), *system_program.key)?;

        let balance_pre = permission_data.balance;
        let bid_amount = permission_data.bid_amount;

        if permission.lamports() < balance_pre + bid_amount {
            return Err(ExpressRelayError::BidNotMet.into());
        }

        let fee_protocol = bid_amount * metadata_data.split_protocol / metadata_data.split_precision;
        let fee_relayer = (bid_amount - fee_protocol) * metadata_data.split_relayer / metadata_data.split_precision;

        Self::transfer_lamports(permission, protocol, fee_protocol)?;
        Self::transfer_lamports(permission, relayer_fee_receiver, fee_relayer)?;

        // close permission account
        permission.data.borrow_mut().fill(0);
        Self::transfer_lamports(permission, relayer_signer, permission.lamports())?;

        Ok(())
    }

    fn transfer_lamports(
        from: &AccountInfo,
        to: &AccountInfo,
        amount: u64,
    ) -> ProgramResult {
        if **from.try_borrow_lamports()? < amount {
            return Err(ProgramError::InsufficientFunds.into());
        }
        **from.try_borrow_mut_lamports()? -= amount;
        **to.try_borrow_mut_lamports()? += amount;
        Ok(())
    }
}

// TODO: refactor all the validation code
// TODO: use anchor
// TODO: check instruction index with sysvar account, check if relayer is used anywhere else
