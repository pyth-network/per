use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::program_error::ProgramError;

#[repr(C)]
#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug, Clone)]
pub struct InitializeArgs {
    pub split_protocol: u64,
    pub split_relayer: u64,
    pub split_precision: u64,
}

#[repr(C)]
#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug, Clone)]
pub struct SetRelayerArgs {}

#[repr(C)]
#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug, Clone)]
pub struct SetSplitsArgs {
    pub split_protocol: u64,
    pub split_relayer: u64,
    pub split_precision: u64,
}

#[repr(C)]
#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug, Clone)]
pub struct PermissionArgs{
    pub permission_id: Box<[u8]>,
    pub bid_id: [u8; 16],
    pub bid_amount: u64,
}

#[repr(C)]
#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug, Clone)]
pub struct DepermissionArgs{
    pub permission_id: Box<[u8]>,
    pub bid_id: [u8; 16],
}

pub enum ExpressRelayInstruction {

    /*
    Accounts expected:
    0. [Writable, Signer] payer
    1. [Writable] express relay metadata
    2. admin
    3. relayer signer
    4. relayer fee receiver
    5. system program
    */
    Initialize,

    /*
    Accounts expected:
    0. [Writable, Signer] admin
    1. [Writable] express relay metadata
    2. relayer signer
    3. relayer fee receiver
    */
    SetRelayer,

    /*
    Accounts expected:
    0. [Writable, Signer] admin
    1. [Writable] express relay metadata
    */
    SetSplits,

    /*
    Accounts expected:
    0. [Writable, Signer] relayer signer
    1. [Writable] permission account
    2. protocol
    3. express relay metadata
    4. system program
    */
    Permission,

    /*
    Accounts expected:
    0. [Writable, Signer] relayer signer
    1. [Writable] permission account
    2. [Writable] protocol
    3. [Writable] relayer fee receiver
    4. express relay metadata
    5. system program
    */
    Depermission,
}

impl ExpressRelayInstruction {
    pub fn unpack(tag: &u8) -> Result<Self, ProgramError> {
        Ok(match tag {
            0 => Self::Permission,
            1 => Self::Depermission,
            _ => return Err(ProgramError::InvalidInstructionData),
        })
    }
}
