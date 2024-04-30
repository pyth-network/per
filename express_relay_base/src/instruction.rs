use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::program_error::ProgramError;

#[repr(C)]
#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug, Clone)]
pub struct InitializeArgs {
    pub split_protocol_default: u64,
    pub split_relayer: u64,
}
pub const INDEX_INITIALIZE: u8 = 0;

#[repr(C)]
#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug, Clone)]
pub struct SetRelayerArgs {}
pub const INDEX_SET_RELAYER: u8 = 1;

#[repr(C)]
#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug, Clone)]
pub struct SetSplitsArgs {
    pub split_protocol_default: u64,
    pub split_relayer: u64,
    pub split_precision: u64,
}
pub const INDEX_SET_SPLITS: u8 = 2;

#[repr(C)]
#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug, Clone)]
pub struct SetProtocolSplitArgs {
    pub split_protocol: u64,
}
pub const INDEX_SET_PROTOCOL_SPLIT: u8 = 3;

#[repr(C)]
#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug, Clone)]
pub struct PermissionArgs{
    pub permission_id: Box<[u8]>,
    pub bid_id: [u8; 16],
    pub bid_amount: u64,
}
pub const INDEX_PERMISSION: u8 = 4;

#[repr(C)]
#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug, Clone)]
pub struct DepermissionArgs{
    pub permission_id: Box<[u8]>,
    pub bid_id: [u8; 16],
}
pub const INDEX_DEPERMISSION: u8 = 5;

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
    0. [Writable, Signer] admin
    1. [Writable] protocol config
    2. express relay metadata
    3. protocol
    4. system program
    */
    SetProtocolSplit,

    /*
    Accounts expected:
    0. [Writable, Signer] relayer signer
    1. [Writable] permission account
    2. protocol
    3. express relay metadata
    4. system program
    5. sysvar instructions
    */
    Permission,

    /*
    Accounts expected:
    0. [Writable, Signer] relayer signer
    1. [Writable] permission account
    2. [Writable] protocol
    3. [Writable] relayer fee receiver
    4. protocol config
    5. express relay metadata
    6. system program
    7. sysvar instructions
    */
    Depermission,
}

impl ExpressRelayInstruction {
    pub fn unpack(tag: &u8) -> Result<Self, ProgramError> {
        Ok(match tag {
            &INDEX_INITIALIZE => Self::Initialize,
            &INDEX_SET_RELAYER => Self::SetRelayer,
            &INDEX_SET_SPLITS => Self::SetSplits,
            &INDEX_SET_PROTOCOL_SPLIT => Self::SetProtocolSplit,
            &INDEX_PERMISSION => Self::Permission,
            &INDEX_DEPERMISSION => Self::Depermission,
            _ => return Err(ProgramError::InvalidInstructionData),
        })
    }
}
