#![feature(prelude_import)]
#[prelude_import]
use std::prelude::rust_2021::*;
#[macro_use]
extern crate std;
pub mod error {
    use anchor_lang::prelude::*;
    #[repr(u32)]
    pub enum ErrorCode {
        FeeSplitLargerThanPrecision,
        FeesHigherThanBid,
        DeadlinePassed,
        InvalidCPISubmitBid,
        MissingPermission,
        MultiplePermissions,
        InsufficientSearcherFunds,
        InsufficientRent,
        InvalidAta,
        InvalidMint,
    }
    #[automatically_derived]
    impl ::core::fmt::Debug for ErrorCode {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            ::core::fmt::Formatter::write_str(
                f,
                match self {
                    ErrorCode::FeeSplitLargerThanPrecision => {
                        "FeeSplitLargerThanPrecision"
                    }
                    ErrorCode::FeesHigherThanBid => "FeesHigherThanBid",
                    ErrorCode::DeadlinePassed => "DeadlinePassed",
                    ErrorCode::InvalidCPISubmitBid => "InvalidCPISubmitBid",
                    ErrorCode::MissingPermission => "MissingPermission",
                    ErrorCode::MultiplePermissions => "MultiplePermissions",
                    ErrorCode::InsufficientSearcherFunds => "InsufficientSearcherFunds",
                    ErrorCode::InsufficientRent => "InsufficientRent",
                    ErrorCode::InvalidAta => "InvalidAta",
                    ErrorCode::InvalidMint => "InvalidMint",
                },
            )
        }
    }
    #[automatically_derived]
    impl ::core::clone::Clone for ErrorCode {
        #[inline]
        fn clone(&self) -> ErrorCode {
            *self
        }
    }
    #[automatically_derived]
    impl ::core::marker::Copy for ErrorCode {}
    impl ErrorCode {
        /// Gets the name of this [#enum_name].
        pub fn name(&self) -> String {
            match self {
                ErrorCode::FeeSplitLargerThanPrecision => {
                    "FeeSplitLargerThanPrecision".to_string()
                }
                ErrorCode::FeesHigherThanBid => "FeesHigherThanBid".to_string(),
                ErrorCode::DeadlinePassed => "DeadlinePassed".to_string(),
                ErrorCode::InvalidCPISubmitBid => "InvalidCPISubmitBid".to_string(),
                ErrorCode::MissingPermission => "MissingPermission".to_string(),
                ErrorCode::MultiplePermissions => "MultiplePermissions".to_string(),
                ErrorCode::InsufficientSearcherFunds => {
                    "InsufficientSearcherFunds".to_string()
                }
                ErrorCode::InsufficientRent => "InsufficientRent".to_string(),
                ErrorCode::InvalidAta => "InvalidAta".to_string(),
                ErrorCode::InvalidMint => "InvalidMint".to_string(),
            }
        }
    }
    impl From<ErrorCode> for u32 {
        fn from(e: ErrorCode) -> u32 {
            e as u32 + anchor_lang::error::ERROR_CODE_OFFSET
        }
    }
    impl From<ErrorCode> for anchor_lang::error::Error {
        fn from(error_code: ErrorCode) -> anchor_lang::error::Error {
            anchor_lang::error::Error::from(anchor_lang::error::AnchorError {
                error_name: error_code.name(),
                error_code_number: error_code.into(),
                error_msg: error_code.to_string(),
                error_origin: None,
                compared_values: None,
            })
        }
    }
    impl std::fmt::Display for ErrorCode {
        fn fmt(
            &self,
            fmt: &mut std::fmt::Formatter<'_>,
        ) -> std::result::Result<(), std::fmt::Error> {
            match self {
                ErrorCode::FeeSplitLargerThanPrecision => {
                    fmt.write_fmt(format_args!("Fee split(s) larger than fee precision"))
                }
                ErrorCode::FeesHigherThanBid => {
                    fmt.write_fmt(format_args!("Fees higher than bid"))
                }
                ErrorCode::DeadlinePassed => {
                    fmt.write_fmt(format_args!("Deadline passed"))
                }
                ErrorCode::InvalidCPISubmitBid => {
                    fmt.write_fmt(
                        format_args!("Invalid CPI into submit bid instruction"),
                    )
                }
                ErrorCode::MissingPermission => {
                    fmt.write_fmt(format_args!("Missing permission"))
                }
                ErrorCode::MultiplePermissions => {
                    fmt.write_fmt(format_args!("Multiple permissions"))
                }
                ErrorCode::InsufficientSearcherFunds => {
                    fmt.write_fmt(format_args!("Insufficient searcher funds"))
                }
                ErrorCode::InsufficientRent => {
                    fmt.write_fmt(format_args!("Insufficient funds for rent"))
                }
                ErrorCode::InvalidAta => {
                    fmt.write_fmt(format_args!("Invalid ATA provided"))
                }
                ErrorCode::InvalidMint => {
                    fmt.write_fmt(format_args!("A token account has the wrong mint"))
                }
            }
        }
    }
}
pub mod sdk {
    pub mod cpi {
        use {
            crate::{
                __cpi_client_accounts_check_permission::CheckPermission,
                cpi::check_permission,
            },
            anchor_lang::prelude::*,
        };
        /// Makes a CPI call to the `CheckPermission` instruction in the Express Relay program.
        /// Permissioning takes the form of a `SubmitBid` instruction with matching permission and router accounts.
        /// Returns the fees paid to the router in the matching instructions.
        pub fn check_permission_cpi<'info>(
            check_permission_accounts: CheckPermission<'info>,
            express_relay_program: AccountInfo<'info>,
        ) -> Result<u64> {
            let result = check_permission(
                CpiContext::new(
                    express_relay_program.to_account_info(),
                    check_permission_accounts,
                ),
            )?;
            let fees_router = result.get();
            Ok(fees_router)
        }
    }
    pub mod test_helpers {
        use {
            crate::{
                accounts, instruction, InitializeArgs, SubmitBidArgs,
                ID as EXPRESS_RELAY_PID, SEED_CONFIG_ROUTER, SEED_METADATA,
            },
            anchor_lang::{
                prelude::*,
                solana_program::{
                    instruction::Instruction, sysvar::instructions as sysvar_instructions,
                },
                system_program, InstructionData,
            },
        };
        /// Helper method to create an instruction to initialize the express relay program.
        /// Should be able to sign transactions with the secret keys of the provided payer and `relayer_signer`.
        /// The fee split is set to 100% for the router, since fee payments to relayer are not important for the integrating program's tests.
        /// Instead it is more important for the integrating program to ensure their router account has enough rent to avoid `InsufficientRent` error.
        pub fn create_initialize_express_relay_ix(
            payer: Pubkey,
            admin: Pubkey,
            relayer_signer: Pubkey,
            fee_receiver_relayer: Pubkey,
        ) -> Instruction {
            let express_relay_metadata = Pubkey::find_program_address(
                    &[SEED_METADATA],
                    &EXPRESS_RELAY_PID,
                )
                .0;
            let split_router_default = 10000;
            let split_relayer = 0;
            let accounts_initialize = accounts::Initialize {
                payer,
                express_relay_metadata,
                admin,
                relayer_signer,
                fee_receiver_relayer,
                system_program: system_program::ID,
            }
                .to_account_metas(None);
            let data_initialize = instruction::Initialize {
                data: InitializeArgs {
                    split_router_default,
                    split_relayer,
                },
            }
                .data();
            Instruction {
                program_id: EXPRESS_RELAY_PID,
                accounts: accounts_initialize,
                data: data_initialize,
            }
        }
        /// Creates and adds to the provided instructions a `SubmitBid` instruction.
        pub fn add_express_relay_submit_bid_instruction(
            ixs: &mut Vec<Instruction>,
            searcher: Pubkey,
            relayer_signer: Pubkey,
            fee_receiver_relayer: Pubkey,
            permission: Pubkey,
            router: Pubkey,
            bid_amount: u64,
        ) -> Vec<Instruction> {
            let deadline = i64::MAX;
            let ix_submit_bid = create_submit_bid_instruction(
                searcher,
                relayer_signer,
                fee_receiver_relayer,
                permission,
                router,
                deadline,
                bid_amount,
            );
            ixs.push(ix_submit_bid);
            ixs.to_vec()
        }
        pub fn create_submit_bid_instruction(
            searcher: Pubkey,
            relayer_signer: Pubkey,
            fee_receiver_relayer: Pubkey,
            permission: Pubkey,
            router: Pubkey,
            deadline: i64,
            bid_amount: u64,
        ) -> Instruction {
            let config_router = Pubkey::find_program_address(
                    &[SEED_CONFIG_ROUTER, router.as_ref()],
                    &EXPRESS_RELAY_PID,
                )
                .0;
            let express_relay_metadata = Pubkey::find_program_address(
                    &[SEED_METADATA],
                    &EXPRESS_RELAY_PID,
                )
                .0;
            let accounts_submit_bid = accounts::SubmitBid {
                searcher,
                relayer_signer,
                permission,
                router,
                config_router,
                fee_receiver_relayer,
                express_relay_metadata,
                system_program: system_program::ID,
                sysvar_instructions: sysvar_instructions::ID,
            }
                .to_account_metas(None);
            let data_submit_bid = instruction::SubmitBid {
                data: SubmitBidArgs {
                    deadline,
                    bid_amount,
                },
            }
                .data();
            Instruction {
                program_id: EXPRESS_RELAY_PID,
                accounts: accounts_submit_bid,
                data: data_submit_bid,
            }
        }
    }
}
pub mod state {
    use anchor_lang::prelude::*;
    pub const FEE_SPLIT_PRECISION: u64 = 10_000;
    pub const RESERVE_EXPRESS_RELAY_METADATA: usize = 8 + 112 + 300;
    pub const SEED_METADATA: &[u8] = b"metadata";
    pub struct ExpressRelayMetadata {
        pub admin: Pubkey,
        pub relayer_signer: Pubkey,
        pub fee_receiver_relayer: Pubkey,
        pub split_router_default: u64,
        pub split_relayer: u64,
        pub split_protocol: u64,
    }
    #[automatically_derived]
    impl ::core::default::Default for ExpressRelayMetadata {
        #[inline]
        fn default() -> ExpressRelayMetadata {
            ExpressRelayMetadata {
                admin: ::core::default::Default::default(),
                relayer_signer: ::core::default::Default::default(),
                fee_receiver_relayer: ::core::default::Default::default(),
                split_router_default: ::core::default::Default::default(),
                split_relayer: ::core::default::Default::default(),
                split_protocol: ::core::default::Default::default(),
            }
        }
    }
    impl borsh::ser::BorshSerialize for ExpressRelayMetadata
    where
        Pubkey: borsh::ser::BorshSerialize,
        Pubkey: borsh::ser::BorshSerialize,
        Pubkey: borsh::ser::BorshSerialize,
        u64: borsh::ser::BorshSerialize,
        u64: borsh::ser::BorshSerialize,
        u64: borsh::ser::BorshSerialize,
    {
        fn serialize<W: borsh::maybestd::io::Write>(
            &self,
            writer: &mut W,
        ) -> ::core::result::Result<(), borsh::maybestd::io::Error> {
            borsh::BorshSerialize::serialize(&self.admin, writer)?;
            borsh::BorshSerialize::serialize(&self.relayer_signer, writer)?;
            borsh::BorshSerialize::serialize(&self.fee_receiver_relayer, writer)?;
            borsh::BorshSerialize::serialize(&self.split_router_default, writer)?;
            borsh::BorshSerialize::serialize(&self.split_relayer, writer)?;
            borsh::BorshSerialize::serialize(&self.split_protocol, writer)?;
            Ok(())
        }
    }
    impl borsh::de::BorshDeserialize for ExpressRelayMetadata
    where
        Pubkey: borsh::BorshDeserialize,
        Pubkey: borsh::BorshDeserialize,
        Pubkey: borsh::BorshDeserialize,
        u64: borsh::BorshDeserialize,
        u64: borsh::BorshDeserialize,
        u64: borsh::BorshDeserialize,
    {
        fn deserialize_reader<R: borsh::maybestd::io::Read>(
            reader: &mut R,
        ) -> ::core::result::Result<Self, borsh::maybestd::io::Error> {
            Ok(Self {
                admin: borsh::BorshDeserialize::deserialize_reader(reader)?,
                relayer_signer: borsh::BorshDeserialize::deserialize_reader(reader)?,
                fee_receiver_relayer: borsh::BorshDeserialize::deserialize_reader(
                    reader,
                )?,
                split_router_default: borsh::BorshDeserialize::deserialize_reader(
                    reader,
                )?,
                split_relayer: borsh::BorshDeserialize::deserialize_reader(reader)?,
                split_protocol: borsh::BorshDeserialize::deserialize_reader(reader)?,
            })
        }
    }
    #[automatically_derived]
    impl ::core::clone::Clone for ExpressRelayMetadata {
        #[inline]
        fn clone(&self) -> ExpressRelayMetadata {
            ExpressRelayMetadata {
                admin: ::core::clone::Clone::clone(&self.admin),
                relayer_signer: ::core::clone::Clone::clone(&self.relayer_signer),
                fee_receiver_relayer: ::core::clone::Clone::clone(
                    &self.fee_receiver_relayer,
                ),
                split_router_default: ::core::clone::Clone::clone(
                    &self.split_router_default,
                ),
                split_relayer: ::core::clone::Clone::clone(&self.split_relayer),
                split_protocol: ::core::clone::Clone::clone(&self.split_protocol),
            }
        }
    }
    #[automatically_derived]
    impl anchor_lang::AccountSerialize for ExpressRelayMetadata {
        fn try_serialize<W: std::io::Write>(
            &self,
            writer: &mut W,
        ) -> anchor_lang::Result<()> {
            if writer.write_all(ExpressRelayMetadata::DISCRIMINATOR).is_err() {
                return Err(anchor_lang::error::ErrorCode::AccountDidNotSerialize.into());
            }
            if AnchorSerialize::serialize(self, writer).is_err() {
                return Err(anchor_lang::error::ErrorCode::AccountDidNotSerialize.into());
            }
            Ok(())
        }
    }
    #[automatically_derived]
    impl anchor_lang::AccountDeserialize for ExpressRelayMetadata {
        fn try_deserialize(buf: &mut &[u8]) -> anchor_lang::Result<Self> {
            if buf.len() < ExpressRelayMetadata::DISCRIMINATOR.len() {
                return Err(
                    anchor_lang::error::ErrorCode::AccountDiscriminatorNotFound.into(),
                );
            }
            let given_disc = &buf[..ExpressRelayMetadata::DISCRIMINATOR.len()];
            if ExpressRelayMetadata::DISCRIMINATOR != given_disc {
                return Err(
                    anchor_lang::error::Error::from(anchor_lang::error::AnchorError {
                            error_name: anchor_lang::error::ErrorCode::AccountDiscriminatorMismatch
                                .name(),
                            error_code_number: anchor_lang::error::ErrorCode::AccountDiscriminatorMismatch
                                .into(),
                            error_msg: anchor_lang::error::ErrorCode::AccountDiscriminatorMismatch
                                .to_string(),
                            error_origin: Some(
                                anchor_lang::error::ErrorOrigin::Source(anchor_lang::error::Source {
                                    filename: "programs/express_relay/src/state.rs",
                                    line: 8u32,
                                }),
                            ),
                            compared_values: None,
                        })
                        .with_account_name("ExpressRelayMetadata"),
                );
            }
            Self::try_deserialize_unchecked(buf)
        }
        fn try_deserialize_unchecked(buf: &mut &[u8]) -> anchor_lang::Result<Self> {
            let mut data: &[u8] = &buf[ExpressRelayMetadata::DISCRIMINATOR.len()..];
            AnchorDeserialize::deserialize(&mut data)
                .map_err(|_| {
                    anchor_lang::error::ErrorCode::AccountDidNotDeserialize.into()
                })
        }
    }
    #[automatically_derived]
    impl anchor_lang::Discriminator for ExpressRelayMetadata {
        const DISCRIMINATOR: &'static [u8] = &[204, 75, 133, 7, 175, 241, 130, 11];
    }
    #[automatically_derived]
    impl anchor_lang::Owner for ExpressRelayMetadata {
        fn owner() -> Pubkey {
            crate::ID
        }
    }
    pub const RESERVE_EXPRESS_RELAY_CONFIG_ROUTER: usize = 8 + 40 + 200;
    pub const SEED_CONFIG_ROUTER: &[u8] = b"config_router";
    pub struct ConfigRouter {
        pub router: Pubkey,
        pub split: u64,
    }
    #[automatically_derived]
    impl ::core::default::Default for ConfigRouter {
        #[inline]
        fn default() -> ConfigRouter {
            ConfigRouter {
                router: ::core::default::Default::default(),
                split: ::core::default::Default::default(),
            }
        }
    }
    impl borsh::ser::BorshSerialize for ConfigRouter
    where
        Pubkey: borsh::ser::BorshSerialize,
        u64: borsh::ser::BorshSerialize,
    {
        fn serialize<W: borsh::maybestd::io::Write>(
            &self,
            writer: &mut W,
        ) -> ::core::result::Result<(), borsh::maybestd::io::Error> {
            borsh::BorshSerialize::serialize(&self.router, writer)?;
            borsh::BorshSerialize::serialize(&self.split, writer)?;
            Ok(())
        }
    }
    impl borsh::de::BorshDeserialize for ConfigRouter
    where
        Pubkey: borsh::BorshDeserialize,
        u64: borsh::BorshDeserialize,
    {
        fn deserialize_reader<R: borsh::maybestd::io::Read>(
            reader: &mut R,
        ) -> ::core::result::Result<Self, borsh::maybestd::io::Error> {
            Ok(Self {
                router: borsh::BorshDeserialize::deserialize_reader(reader)?,
                split: borsh::BorshDeserialize::deserialize_reader(reader)?,
            })
        }
    }
    #[automatically_derived]
    impl ::core::clone::Clone for ConfigRouter {
        #[inline]
        fn clone(&self) -> ConfigRouter {
            ConfigRouter {
                router: ::core::clone::Clone::clone(&self.router),
                split: ::core::clone::Clone::clone(&self.split),
            }
        }
    }
    #[automatically_derived]
    impl anchor_lang::AccountSerialize for ConfigRouter {
        fn try_serialize<W: std::io::Write>(
            &self,
            writer: &mut W,
        ) -> anchor_lang::Result<()> {
            if writer.write_all(ConfigRouter::DISCRIMINATOR).is_err() {
                return Err(anchor_lang::error::ErrorCode::AccountDidNotSerialize.into());
            }
            if AnchorSerialize::serialize(self, writer).is_err() {
                return Err(anchor_lang::error::ErrorCode::AccountDidNotSerialize.into());
            }
            Ok(())
        }
    }
    #[automatically_derived]
    impl anchor_lang::AccountDeserialize for ConfigRouter {
        fn try_deserialize(buf: &mut &[u8]) -> anchor_lang::Result<Self> {
            if buf.len() < ConfigRouter::DISCRIMINATOR.len() {
                return Err(
                    anchor_lang::error::ErrorCode::AccountDiscriminatorNotFound.into(),
                );
            }
            let given_disc = &buf[..ConfigRouter::DISCRIMINATOR.len()];
            if ConfigRouter::DISCRIMINATOR != given_disc {
                return Err(
                    anchor_lang::error::Error::from(anchor_lang::error::AnchorError {
                            error_name: anchor_lang::error::ErrorCode::AccountDiscriminatorMismatch
                                .name(),
                            error_code_number: anchor_lang::error::ErrorCode::AccountDiscriminatorMismatch
                                .into(),
                            error_msg: anchor_lang::error::ErrorCode::AccountDiscriminatorMismatch
                                .to_string(),
                            error_origin: Some(
                                anchor_lang::error::ErrorOrigin::Source(anchor_lang::error::Source {
                                    filename: "programs/express_relay/src/state.rs",
                                    line: 24u32,
                                }),
                            ),
                            compared_values: None,
                        })
                        .with_account_name("ConfigRouter"),
                );
            }
            Self::try_deserialize_unchecked(buf)
        }
        fn try_deserialize_unchecked(buf: &mut &[u8]) -> anchor_lang::Result<Self> {
            let mut data: &[u8] = &buf[ConfigRouter::DISCRIMINATOR.len()..];
            AnchorDeserialize::deserialize(&mut data)
                .map_err(|_| {
                    anchor_lang::error::ErrorCode::AccountDidNotDeserialize.into()
                })
        }
    }
    #[automatically_derived]
    impl anchor_lang::Discriminator for ConfigRouter {
        const DISCRIMINATOR: &'static [u8] = &[135, 66, 240, 166, 94, 198, 187, 36];
    }
    #[automatically_derived]
    impl anchor_lang::Owner for ConfigRouter {
        fn owner() -> Pubkey {
            crate::ID
        }
    }
}
pub mod token {
    use {
        crate::error::ErrorCode, anchor_lang::prelude::*,
        anchor_spl::{
            associated_token::get_associated_token_address,
            token_interface::{self, Mint, TokenAccount, TokenInterface, TransferChecked},
        },
    };
    pub fn transfer_token_if_needed<'info>(
        from: &InterfaceAccount<'info, TokenAccount>,
        to: &InterfaceAccount<'info, TokenAccount>,
        token_program: &Interface<'info, TokenInterface>,
        authority: &Signer<'info>,
        mint: &InterfaceAccount<'info, Mint>,
        amount: u64,
    ) -> Result<()> {
        if amount > 0 {
            let cpi_accounts = TransferChecked {
                from: from.to_account_info(),
                to: to.to_account_info(),
                mint: mint.to_account_info(),
                authority: authority.to_account_info(),
            };
            token_interface::transfer_checked(
                CpiContext::new(token_program.to_account_info(), cpi_accounts),
                amount,
                mint.decimals,
            )?;
        }
        Ok(())
    }
    pub fn check_mint<'info>(
        ta: &InterfaceAccount<'info, TokenAccount>,
        mint: &InterfaceAccount<'info, Mint>,
    ) -> Result<()> {
        if !(ta.mint == mint.key()) {
            return Err(
                anchor_lang::error::Error::from(anchor_lang::error::AnchorError {
                    error_name: ErrorCode::InvalidMint.name(),
                    error_code_number: ErrorCode::InvalidMint.into(),
                    error_msg: ErrorCode::InvalidMint.to_string(),
                    error_origin: Some(
                        anchor_lang::error::ErrorOrigin::Source(anchor_lang::error::Source {
                            filename: "programs/express_relay/src/token.rs",
                            line: 45u32,
                        }),
                    ),
                    compared_values: None,
                }),
            );
        }
        Ok(())
    }
    pub fn check_ata<'info>(
        ata: &InterfaceAccount<'info, TokenAccount>,
        owner: &Pubkey,
        mint: &InterfaceAccount<'info, Mint>,
    ) -> Result<()> {
        if !(ata.key() == get_associated_token_address(owner, &mint.key())) {
            return Err(
                anchor_lang::error::Error::from(anchor_lang::error::AnchorError {
                    error_name: ErrorCode::InvalidAta.name(),
                    error_code_number: ErrorCode::InvalidAta.into(),
                    error_msg: ErrorCode::InvalidAta.to_string(),
                    error_origin: Some(
                        anchor_lang::error::ErrorOrigin::Source(anchor_lang::error::Source {
                            filename: "programs/express_relay/src/token.rs",
                            line: 54u32,
                        }),
                    ),
                    compared_values: None,
                }),
            );
        }
        Ok(())
    }
}
pub mod utils {
    use {
        crate::{error::ErrorCode, state::*, SubmitBid, SubmitBidArgs},
        anchor_lang::{
            prelude::*,
            solana_program::{
                instruction::Instruction, serialize_utils::read_u16,
                sysvar::instructions::load_instruction_at_checked,
            },
            system_program::{transfer, Transfer},
            Discriminator,
        },
    };
    pub fn validate_fee_split(split: u64) -> Result<()> {
        if split > FEE_SPLIT_PRECISION {
            return Err(
                anchor_lang::error::Error::from(anchor_lang::error::AnchorError {
                    error_name: ErrorCode::FeeSplitLargerThanPrecision.name(),
                    error_code_number: ErrorCode::FeeSplitLargerThanPrecision.into(),
                    error_msg: ErrorCode::FeeSplitLargerThanPrecision.to_string(),
                    error_origin: Some(
                        anchor_lang::error::ErrorOrigin::Source(anchor_lang::error::Source {
                            filename: "programs/express_relay/src/utils.rs",
                            line: 25u32,
                        }),
                    ),
                    compared_values: None,
                }),
            );
        }
        Ok(())
    }
    pub fn transfer_lamports(
        from: &AccountInfo,
        to: &AccountInfo,
        amount: u64,
    ) -> Result<()> {
        **from.try_borrow_mut_lamports()? -= amount;
        **to.try_borrow_mut_lamports()? += amount;
        Ok(())
    }
    pub fn transfer_lamports_cpi<'info>(
        from: &AccountInfo<'info>,
        to: &AccountInfo<'info>,
        amount: u64,
        system_program: AccountInfo<'info>,
    ) -> Result<()> {
        let cpi_accounts = Transfer {
            from: from.clone(),
            to: to.clone(),
        };
        transfer(CpiContext::new(system_program, cpi_accounts), amount)?;
        Ok(())
    }
    pub fn check_fee_hits_min_rent(account: &AccountInfo, fee: u64) -> Result<()> {
        let balance = account.lamports();
        let rent = Rent::get()?.minimum_balance(account.data_len());
        if balance.checked_add(fee).ok_or(ProgramError::ArithmeticOverflow)? < rent {
            return Err(
                anchor_lang::error::Error::from(anchor_lang::error::AnchorError {
                    error_name: ErrorCode::InsufficientRent.name(),
                    error_code_number: ErrorCode::InsufficientRent.into(),
                    error_msg: ErrorCode::InsufficientRent.to_string(),
                    error_origin: Some(
                        anchor_lang::error::ErrorOrigin::Source(anchor_lang::error::Source {
                            filename: "programs/express_relay/src/utils.rs",
                            line: 60u32,
                        }),
                    ),
                    compared_values: None,
                }),
            );
        }
        Ok(())
    }
    pub struct PermissionInfo<'info> {
        pub permission: Pubkey,
        pub router: Pubkey,
        pub config_router: AccountInfo<'info>,
        pub express_relay_metadata: AccountInfo<'info>,
    }
    /// Performs instruction introspection to retrieve a vector of `SubmitBid` instructions in the current transaction.
    /// If `permission_info` is specified, only instructions with matching permission and router accounts are returned.
    pub fn get_matching_submit_bid_instructions(
        sysvar_instructions: AccountInfo,
        permission_info: Option<&PermissionInfo>,
    ) -> Result<Vec<Instruction>> {
        let num_instructions = read_u16(&mut 0, &sysvar_instructions.data.borrow())
            .map_err(|_| ProgramError::InvalidInstructionData)?;
        let mut matching_instructions = Vec::new();
        for index in 0..num_instructions {
            let ix = load_instruction_at_checked(index.into(), &sysvar_instructions)?;
            if ix.program_id != crate::id() {
                continue;
            }
            if ix.data[0..8] != *crate::instruction::SubmitBid::DISCRIMINATOR {
                continue;
            }
            if let Some(permission_info) = permission_info {
                if ix.accounts[2].pubkey != permission_info.permission {
                    continue;
                }
                if ix.accounts[3].pubkey != permission_info.router {
                    continue;
                }
                if ix.accounts[4].pubkey != permission_info.config_router.key() {
                    continue;
                }
                if ix.accounts[5].pubkey != permission_info.express_relay_metadata.key()
                {
                    continue;
                }
            }
            matching_instructions.push(ix);
        }
        Ok(matching_instructions)
    }
    /// Extracts the bid paid from a `SubmitBid` instruction.
    pub fn extract_bid_from_submit_bid_ix(submit_bid_ix: &Instruction) -> Result<u64> {
        let submit_bid_args = SubmitBidArgs::try_from_slice(
                &submit_bid_ix.data[crate::instruction::SubmitBid::DISCRIMINATOR.len()..],
            )
            .map_err(|_| ProgramError::BorshIoError(
                "Failed to deserialize SubmitBidArgs".to_string(),
            ))?;
        Ok(submit_bid_args.bid_amount)
    }
    /// Computes the fee to pay the router based on the specified `bid_amount` and the `split_router`.
    fn perform_fee_split_router(bid_amount: u64, split_router: u64) -> Result<u64> {
        let fee_router = bid_amount
            .checked_mul(split_router)
            .ok_or(ProgramError::ArithmeticOverflow)? / FEE_SPLIT_PRECISION;
        if fee_router > bid_amount {
            return Err(
                anchor_lang::error::Error::from(anchor_lang::error::AnchorError {
                    error_name: ErrorCode::FeesHigherThanBid.name(),
                    error_code_number: ErrorCode::FeesHigherThanBid.into(),
                    error_msg: ErrorCode::FeesHigherThanBid.to_string(),
                    error_origin: Some(
                        anchor_lang::error::ErrorOrigin::Source(anchor_lang::error::Source {
                            filename: "programs/express_relay/src/utils.rs",
                            line: 133u32,
                        }),
                    ),
                    compared_values: None,
                }),
            );
        }
        Ok(fee_router)
    }
    /// Performs fee splits on a bid amount.
    /// Returns amount to pay to router and amount to pay to relayer.
    pub fn perform_fee_splits(
        bid_amount: u64,
        split_router: u64,
        split_relayer: u64,
    ) -> Result<(u64, u64)> {
        let fee_router = perform_fee_split_router(bid_amount, split_router)?;
        let fee_relayer = bid_amount
            .saturating_sub(fee_router)
            .checked_mul(split_relayer)
            .ok_or(ProgramError::ArithmeticOverflow)? / FEE_SPLIT_PRECISION;
        if fee_relayer.checked_add(fee_router).ok_or(ProgramError::ArithmeticOverflow)?
            > bid_amount
        {
            return Err(
                anchor_lang::error::Error::from(anchor_lang::error::AnchorError {
                    error_name: ErrorCode::FeesHigherThanBid.name(),
                    error_code_number: ErrorCode::FeesHigherThanBid.into(),
                    error_msg: ErrorCode::FeesHigherThanBid.to_string(),
                    error_origin: Some(
                        anchor_lang::error::ErrorOrigin::Source(anchor_lang::error::Source {
                            filename: "programs/express_relay/src/utils.rs",
                            line: 160u32,
                        }),
                    ),
                    compared_values: None,
                }),
            );
        }
        Ok((fee_router, fee_relayer))
    }
    /// Performs instruction introspection on the current transaction to query SubmitBid instructions that match the specified permission and router.
    /// Returns the number of matching instructions and the total fees paid to the router.
    /// The `config_router` and `express_relay_metadata` accounts passed in `permission_info` are assumed to have already been validated. Note these are not validated in this function.
    pub fn inspect_permissions_in_tx(
        sysvar_instructions: UncheckedAccount,
        permission_info: PermissionInfo,
    ) -> Result<(u16, u64)> {
        let matching_ixs = get_matching_submit_bid_instructions(
            sysvar_instructions.to_account_info(),
            Some(&permission_info),
        )?;
        let n_ixs = matching_ixs.len() as u16;
        let mut total_fees = 0u64;
        let data_config_router = &mut &**permission_info
            .config_router
            .try_borrow_data()?;
        let split_router = match ConfigRouter::try_deserialize(data_config_router) {
            Ok(config_router) => config_router.split,
            Err(_) => {
                let data_express_relay_metadata = &mut &**permission_info
                    .express_relay_metadata
                    .try_borrow_data()?;
                let express_relay_metadata = ExpressRelayMetadata::try_deserialize(
                        data_express_relay_metadata,
                    )
                    .map_err(|_| ProgramError::InvalidAccountData)?;
                express_relay_metadata.split_router_default
            }
        };
        for ix in matching_ixs {
            let bid = extract_bid_from_submit_bid_ix(&ix)?;
            total_fees = total_fees
                .checked_add(perform_fee_split_router(bid, split_router)?)
                .ok_or(ProgramError::ArithmeticOverflow)?;
        }
        Ok((n_ixs, total_fees))
    }
    pub fn handle_bid_payment(ctx: Context<SubmitBid>, bid_amount: u64) -> Result<()> {
        let searcher = &ctx.accounts.searcher;
        let rent_searcher = Rent::get()?
            .minimum_balance(searcher.to_account_info().data_len());
        if bid_amount.checked_add(rent_searcher).ok_or(ProgramError::ArithmeticOverflow)?
            > searcher.lamports()
        {
            return Err(
                anchor_lang::error::Error::from(anchor_lang::error::AnchorError {
                    error_name: ErrorCode::InsufficientSearcherFunds.name(),
                    error_code_number: ErrorCode::InsufficientSearcherFunds.into(),
                    error_msg: ErrorCode::InsufficientSearcherFunds.to_string(),
                    error_origin: Some(
                        anchor_lang::error::ErrorOrigin::Source(anchor_lang::error::Source {
                            filename: "programs/express_relay/src/utils.rs",
                            line: 210u32,
                        }),
                    ),
                    compared_values: None,
                }),
            );
        }
        let express_relay_metadata = &ctx.accounts.express_relay_metadata;
        let split_relayer = express_relay_metadata.split_relayer;
        let split_router_default = express_relay_metadata.split_router_default;
        let config_router = &ctx.accounts.config_router;
        let config_router_account_info = config_router.to_account_info();
        let split_router: u64 = if config_router_account_info.data_len() > 0 {
            let account_data = &mut &**config_router_account_info.try_borrow_data()?;
            let config_router_data = ConfigRouter::try_deserialize(account_data)?;
            config_router_data.split
        } else {
            split_router_default
        };
        let (fee_router, fee_relayer) = perform_fee_splits(
            bid_amount,
            split_router,
            split_relayer,
        )?;
        if fee_router > 0 {
            check_fee_hits_min_rent(&ctx.accounts.router, fee_router)?;
            transfer_lamports_cpi(
                &searcher.to_account_info(),
                &ctx.accounts.router.to_account_info(),
                fee_router,
                ctx.accounts.system_program.to_account_info(),
            )?;
        }
        if fee_relayer > 0 {
            check_fee_hits_min_rent(&ctx.accounts.fee_receiver_relayer, fee_relayer)?;
            transfer_lamports_cpi(
                &searcher.to_account_info(),
                &ctx.accounts.fee_receiver_relayer.to_account_info(),
                fee_relayer,
                ctx.accounts.system_program.to_account_info(),
            )?;
        }
        transfer_lamports_cpi(
            &searcher.to_account_info(),
            &express_relay_metadata.to_account_info(),
            bid_amount.saturating_sub(fee_router).saturating_sub(fee_relayer),
            ctx.accounts.system_program.to_account_info(),
        )?;
        Ok(())
    }
    pub fn perform_fee_split(amount: u64, split_ratio: u64) -> Result<(u64, u64)> {
        let fee = amount
            .checked_mul(split_ratio)
            .ok_or(ProgramError::ArithmeticOverflow)? / 1_000_000;
        Ok((amount.checked_sub(fee).ok_or(ProgramError::ArithmeticOverflow)?, fee))
    }
}
use {
    crate::{error::ErrorCode, state::*, utils::*},
    anchor_lang::{
        prelude::*,
        solana_program::{
            instruction::{get_stack_height, TRANSACTION_LEVEL_STACK_HEIGHT},
            sysvar::instructions as sysvar_instructions,
        },
        system_program::System,
    },
    anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface},
};
/// The static program ID
pub static ID: anchor_lang::solana_program::pubkey::Pubkey = anchor_lang::solana_program::pubkey::Pubkey::new_from_array([
    5u8,
    227u8,
    24u8,
    144u8,
    66u8,
    194u8,
    130u8,
    43u8,
    208u8,
    169u8,
    134u8,
    157u8,
    138u8,
    235u8,
    62u8,
    219u8,
    235u8,
    142u8,
    116u8,
    238u8,
    183u8,
    236u8,
    28u8,
    166u8,
    251u8,
    203u8,
    187u8,
    16u8,
    36u8,
    194u8,
    70u8,
    80u8,
]);
/// Const version of `ID`
pub const ID_CONST: anchor_lang::solana_program::pubkey::Pubkey = anchor_lang::solana_program::pubkey::Pubkey::new_from_array([
    5u8,
    227u8,
    24u8,
    144u8,
    66u8,
    194u8,
    130u8,
    43u8,
    208u8,
    169u8,
    134u8,
    157u8,
    138u8,
    235u8,
    62u8,
    219u8,
    235u8,
    142u8,
    116u8,
    238u8,
    183u8,
    236u8,
    28u8,
    166u8,
    251u8,
    203u8,
    187u8,
    16u8,
    36u8,
    194u8,
    70u8,
    80u8,
]);
/// Confirms that a given pubkey is equivalent to the program ID
pub fn check_id(id: &anchor_lang::solana_program::pubkey::Pubkey) -> bool {
    id == &ID
}
/// Returns the program ID
pub fn id() -> anchor_lang::solana_program::pubkey::Pubkey {
    ID
}
/// Const version of `ID`
pub const fn id_const() -> anchor_lang::solana_program::pubkey::Pubkey {
    ID_CONST
}
use self::express_relay::*;
/// # Safety
#[no_mangle]
pub unsafe extern "C" fn entrypoint(input: *mut u8) -> u64 {
    let (program_id, accounts, instruction_data) = unsafe {
        ::solana_program_entrypoint::deserialize(input)
    };
    match entry(program_id, &accounts, instruction_data) {
        Ok(()) => ::solana_program_entrypoint::SUCCESS,
        Err(error) => error.into(),
    }
}
/// The Anchor codegen exposes a programming model where a user defines
/// a set of methods inside of a `#[program]` module in a way similar
/// to writing RPC request handlers. The macro then generates a bunch of
/// code wrapping these user defined methods into something that can be
/// executed on Solana.
///
/// These methods fall into one category for now.
///
/// Global methods - regular methods inside of the `#[program]`.
///
/// Care must be taken by the codegen to prevent collisions between
/// methods in these different namespaces. For this reason, Anchor uses
/// a variant of sighash to perform method dispatch, rather than
/// something like a simple enum variant discriminator.
///
/// The execution flow of the generated code can be roughly outlined:
///
/// * Start program via the entrypoint.
/// * Check whether the declared program id matches the input program
///   id. If it's not, return an error.
/// * Find and invoke the method based on whether the instruction data
///   starts with the method's discriminator.
/// * Run the method handler wrapper. This wraps the code the user
///   actually wrote, deserializing the accounts, constructing the
///   context, invoking the user's code, and finally running the exit
///   routine, which typically persists account changes.
///
/// The `entry` function here, defines the standard entry to a Solana
/// program, where execution begins.
pub fn entry<'info>(
    program_id: &Pubkey,
    accounts: &'info [AccountInfo<'info>],
    data: &[u8],
) -> anchor_lang::solana_program::entrypoint::ProgramResult {
    try_entry(program_id, accounts, data)
        .map_err(|e| {
            e.log();
            e.into()
        })
}
fn try_entry<'info>(
    program_id: &Pubkey,
    accounts: &'info [AccountInfo<'info>],
    data: &[u8],
) -> anchor_lang::Result<()> {
    if *program_id != ID {
        return Err(anchor_lang::error::ErrorCode::DeclaredProgramIdMismatch.into());
    }
    dispatch(program_id, accounts, data)
}
/// Module representing the program.
pub mod program {
    use super::*;
    /// Type representing the program.
    pub struct ExpressRelay;
    #[automatically_derived]
    impl ::core::clone::Clone for ExpressRelay {
        #[inline]
        fn clone(&self) -> ExpressRelay {
            ExpressRelay
        }
    }
    impl anchor_lang::Id for ExpressRelay {
        fn id() -> Pubkey {
            ID
        }
    }
}
/// Performs method dispatch.
///
/// Each instruction's discriminator is checked until the given instruction data starts with
/// the current discriminator.
///
/// If a match is found, the instruction handler is called using the given instruction data
/// excluding the prepended discriminator bytes.
///
/// If no match is found, the fallback function is executed if it exists, or an error is
/// returned if it doesn't exist.
fn dispatch<'info>(
    program_id: &Pubkey,
    accounts: &'info [AccountInfo<'info>],
    data: &[u8],
) -> anchor_lang::Result<()> {
    if data.starts_with(instruction::Initialize::DISCRIMINATOR) {
        return __private::__global::initialize(
            program_id,
            accounts,
            &data[instruction::Initialize::DISCRIMINATOR.len()..],
        );
    }
    if data.starts_with(instruction::SetAdmin::DISCRIMINATOR) {
        return __private::__global::set_admin(
            program_id,
            accounts,
            &data[instruction::SetAdmin::DISCRIMINATOR.len()..],
        );
    }
    if data.starts_with(instruction::SetRelayer::DISCRIMINATOR) {
        return __private::__global::set_relayer(
            program_id,
            accounts,
            &data[instruction::SetRelayer::DISCRIMINATOR.len()..],
        );
    }
    if data.starts_with(instruction::SetSplits::DISCRIMINATOR) {
        return __private::__global::set_splits(
            program_id,
            accounts,
            &data[instruction::SetSplits::DISCRIMINATOR.len()..],
        );
    }
    if data.starts_with(instruction::SetRouterSplit::DISCRIMINATOR) {
        return __private::__global::set_router_split(
            program_id,
            accounts,
            &data[instruction::SetRouterSplit::DISCRIMINATOR.len()..],
        );
    }
    if data.starts_with(instruction::SubmitBid::DISCRIMINATOR) {
        return __private::__global::submit_bid(
            program_id,
            accounts,
            &data[instruction::SubmitBid::DISCRIMINATOR.len()..],
        );
    }
    if data.starts_with(instruction::CheckPermission::DISCRIMINATOR) {
        return __private::__global::check_permission(
            program_id,
            accounts,
            &data[instruction::CheckPermission::DISCRIMINATOR.len()..],
        );
    }
    if data.starts_with(instruction::WithdrawFees::DISCRIMINATOR) {
        return __private::__global::withdraw_fees(
            program_id,
            accounts,
            &data[instruction::WithdrawFees::DISCRIMINATOR.len()..],
        );
    }
    if data.starts_with(instruction::Swap::DISCRIMINATOR) {
        return __private::__global::swap(
            program_id,
            accounts,
            &data[instruction::Swap::DISCRIMINATOR.len()..],
        );
    }
    if data.starts_with(anchor_lang::idl::IDL_IX_TAG_LE) {
        #[cfg(not(feature = "no-idl"))]
        return __private::__idl::__idl_dispatch(
            program_id,
            accounts,
            &data[anchor_lang::idl::IDL_IX_TAG_LE.len()..],
        );
    }
    if data.starts_with(anchor_lang::event::EVENT_IX_TAG_LE) {
        return Err(anchor_lang::error::ErrorCode::EventInstructionStub.into());
    }
    Err(anchor_lang::error::ErrorCode::InstructionFallbackNotFound.into())
}
/// Create a private module to not clutter the program's namespace.
/// Defines an entrypoint for each individual instruction handler
/// wrapper.
mod __private {
    use super::*;
    /// __idl mod defines handlers for injected Anchor IDL instructions.
    pub mod __idl {
        use super::*;
        #[inline(never)]
        #[cfg(not(feature = "no-idl"))]
        pub fn __idl_dispatch<'info>(
            program_id: &Pubkey,
            accounts: &'info [AccountInfo<'info>],
            idl_ix_data: &[u8],
        ) -> anchor_lang::Result<()> {
            let mut accounts = accounts;
            let mut data: &[u8] = idl_ix_data;
            let ix = anchor_lang::idl::IdlInstruction::deserialize(&mut data)
                .map_err(|_| {
                    anchor_lang::error::ErrorCode::InstructionDidNotDeserialize
                })?;
            match ix {
                anchor_lang::idl::IdlInstruction::Create { data_len } => {
                    let mut bumps = <IdlCreateAccounts as anchor_lang::Bumps>::Bumps::default();
                    let mut reallocs = std::collections::BTreeSet::new();
                    let mut accounts = IdlCreateAccounts::try_accounts(
                        program_id,
                        &mut accounts,
                        &[],
                        &mut bumps,
                        &mut reallocs,
                    )?;
                    __idl_create_account(program_id, &mut accounts, data_len)?;
                    accounts.exit(program_id)?;
                }
                anchor_lang::idl::IdlInstruction::Resize { data_len } => {
                    let mut bumps = <IdlResizeAccount as anchor_lang::Bumps>::Bumps::default();
                    let mut reallocs = std::collections::BTreeSet::new();
                    let mut accounts = IdlResizeAccount::try_accounts(
                        program_id,
                        &mut accounts,
                        &[],
                        &mut bumps,
                        &mut reallocs,
                    )?;
                    __idl_resize_account(program_id, &mut accounts, data_len)?;
                    accounts.exit(program_id)?;
                }
                anchor_lang::idl::IdlInstruction::Close => {
                    let mut bumps = <IdlCloseAccount as anchor_lang::Bumps>::Bumps::default();
                    let mut reallocs = std::collections::BTreeSet::new();
                    let mut accounts = IdlCloseAccount::try_accounts(
                        program_id,
                        &mut accounts,
                        &[],
                        &mut bumps,
                        &mut reallocs,
                    )?;
                    __idl_close_account(program_id, &mut accounts)?;
                    accounts.exit(program_id)?;
                }
                anchor_lang::idl::IdlInstruction::CreateBuffer => {
                    let mut bumps = <IdlCreateBuffer as anchor_lang::Bumps>::Bumps::default();
                    let mut reallocs = std::collections::BTreeSet::new();
                    let mut accounts = IdlCreateBuffer::try_accounts(
                        program_id,
                        &mut accounts,
                        &[],
                        &mut bumps,
                        &mut reallocs,
                    )?;
                    __idl_create_buffer(program_id, &mut accounts)?;
                    accounts.exit(program_id)?;
                }
                anchor_lang::idl::IdlInstruction::Write { data } => {
                    let mut bumps = <IdlAccounts as anchor_lang::Bumps>::Bumps::default();
                    let mut reallocs = std::collections::BTreeSet::new();
                    let mut accounts = IdlAccounts::try_accounts(
                        program_id,
                        &mut accounts,
                        &[],
                        &mut bumps,
                        &mut reallocs,
                    )?;
                    __idl_write(program_id, &mut accounts, data)?;
                    accounts.exit(program_id)?;
                }
                anchor_lang::idl::IdlInstruction::SetAuthority { new_authority } => {
                    let mut bumps = <IdlAccounts as anchor_lang::Bumps>::Bumps::default();
                    let mut reallocs = std::collections::BTreeSet::new();
                    let mut accounts = IdlAccounts::try_accounts(
                        program_id,
                        &mut accounts,
                        &[],
                        &mut bumps,
                        &mut reallocs,
                    )?;
                    __idl_set_authority(program_id, &mut accounts, new_authority)?;
                    accounts.exit(program_id)?;
                }
                anchor_lang::idl::IdlInstruction::SetBuffer => {
                    let mut bumps = <IdlSetBuffer as anchor_lang::Bumps>::Bumps::default();
                    let mut reallocs = std::collections::BTreeSet::new();
                    let mut accounts = IdlSetBuffer::try_accounts(
                        program_id,
                        &mut accounts,
                        &[],
                        &mut bumps,
                        &mut reallocs,
                    )?;
                    __idl_set_buffer(program_id, &mut accounts)?;
                    accounts.exit(program_id)?;
                }
            }
            Ok(())
        }
        use anchor_lang::idl::ERASED_AUTHORITY;
        pub struct IdlAccount {
            pub authority: Pubkey,
            pub data_len: u32,
        }
        #[automatically_derived]
        impl ::core::fmt::Debug for IdlAccount {
            #[inline]
            fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                ::core::fmt::Formatter::debug_struct_field2_finish(
                    f,
                    "IdlAccount",
                    "authority",
                    &self.authority,
                    "data_len",
                    &&self.data_len,
                )
            }
        }
        impl borsh::ser::BorshSerialize for IdlAccount
        where
            Pubkey: borsh::ser::BorshSerialize,
            u32: borsh::ser::BorshSerialize,
        {
            fn serialize<W: borsh::maybestd::io::Write>(
                &self,
                writer: &mut W,
            ) -> ::core::result::Result<(), borsh::maybestd::io::Error> {
                borsh::BorshSerialize::serialize(&self.authority, writer)?;
                borsh::BorshSerialize::serialize(&self.data_len, writer)?;
                Ok(())
            }
        }
        impl borsh::de::BorshDeserialize for IdlAccount
        where
            Pubkey: borsh::BorshDeserialize,
            u32: borsh::BorshDeserialize,
        {
            fn deserialize_reader<R: borsh::maybestd::io::Read>(
                reader: &mut R,
            ) -> ::core::result::Result<Self, borsh::maybestd::io::Error> {
                Ok(Self {
                    authority: borsh::BorshDeserialize::deserialize_reader(reader)?,
                    data_len: borsh::BorshDeserialize::deserialize_reader(reader)?,
                })
            }
        }
        #[automatically_derived]
        impl ::core::clone::Clone for IdlAccount {
            #[inline]
            fn clone(&self) -> IdlAccount {
                IdlAccount {
                    authority: ::core::clone::Clone::clone(&self.authority),
                    data_len: ::core::clone::Clone::clone(&self.data_len),
                }
            }
        }
        #[automatically_derived]
        impl anchor_lang::AccountSerialize for IdlAccount {
            fn try_serialize<W: std::io::Write>(
                &self,
                writer: &mut W,
            ) -> anchor_lang::Result<()> {
                if writer.write_all(IdlAccount::DISCRIMINATOR).is_err() {
                    return Err(
                        anchor_lang::error::ErrorCode::AccountDidNotSerialize.into(),
                    );
                }
                if AnchorSerialize::serialize(self, writer).is_err() {
                    return Err(
                        anchor_lang::error::ErrorCode::AccountDidNotSerialize.into(),
                    );
                }
                Ok(())
            }
        }
        #[automatically_derived]
        impl anchor_lang::AccountDeserialize for IdlAccount {
            fn try_deserialize(buf: &mut &[u8]) -> anchor_lang::Result<Self> {
                if buf.len() < IdlAccount::DISCRIMINATOR.len() {
                    return Err(
                        anchor_lang::error::ErrorCode::AccountDiscriminatorNotFound
                            .into(),
                    );
                }
                let given_disc = &buf[..IdlAccount::DISCRIMINATOR.len()];
                if IdlAccount::DISCRIMINATOR != given_disc {
                    return Err(
                        anchor_lang::error::Error::from(anchor_lang::error::AnchorError {
                                error_name: anchor_lang::error::ErrorCode::AccountDiscriminatorMismatch
                                    .name(),
                                error_code_number: anchor_lang::error::ErrorCode::AccountDiscriminatorMismatch
                                    .into(),
                                error_msg: anchor_lang::error::ErrorCode::AccountDiscriminatorMismatch
                                    .to_string(),
                                error_origin: Some(
                                    anchor_lang::error::ErrorOrigin::Source(anchor_lang::error::Source {
                                        filename: "programs/express_relay/src/lib.rs",
                                        line: 33u32,
                                    }),
                                ),
                                compared_values: None,
                            })
                            .with_account_name("IdlAccount"),
                    );
                }
                Self::try_deserialize_unchecked(buf)
            }
            fn try_deserialize_unchecked(buf: &mut &[u8]) -> anchor_lang::Result<Self> {
                let mut data: &[u8] = &buf[IdlAccount::DISCRIMINATOR.len()..];
                AnchorDeserialize::deserialize(&mut data)
                    .map_err(|_| {
                        anchor_lang::error::ErrorCode::AccountDidNotDeserialize.into()
                    })
            }
        }
        #[automatically_derived]
        impl anchor_lang::Discriminator for IdlAccount {
            const DISCRIMINATOR: &'static [u8] = &[24, 70, 98, 191, 58, 144, 123, 158];
        }
        impl IdlAccount {
            pub fn address(program_id: &Pubkey) -> Pubkey {
                let program_signer = Pubkey::find_program_address(&[], program_id).0;
                Pubkey::create_with_seed(&program_signer, IdlAccount::seed(), program_id)
                    .expect("Seed is always valid")
            }
            pub fn seed() -> &'static str {
                "anchor:idl"
            }
        }
        impl anchor_lang::Owner for IdlAccount {
            fn owner() -> Pubkey {
                crate::ID
            }
        }
        pub struct IdlCreateAccounts<'info> {
            #[account(signer)]
            pub from: AccountInfo<'info>,
            #[account(mut)]
            pub to: AccountInfo<'info>,
            #[account(seeds = [], bump)]
            pub base: AccountInfo<'info>,
            pub system_program: Program<'info, System>,
            #[account(executable)]
            pub program: AccountInfo<'info>,
        }
        #[automatically_derived]
        impl<'info> anchor_lang::Accounts<'info, IdlCreateAccountsBumps>
        for IdlCreateAccounts<'info>
        where
            'info: 'info,
        {
            #[inline(never)]
            fn try_accounts(
                __program_id: &anchor_lang::solana_program::pubkey::Pubkey,
                __accounts: &mut &'info [anchor_lang::solana_program::account_info::AccountInfo<
                    'info,
                >],
                __ix_data: &[u8],
                __bumps: &mut IdlCreateAccountsBumps,
                __reallocs: &mut std::collections::BTreeSet<
                    anchor_lang::solana_program::pubkey::Pubkey,
                >,
            ) -> anchor_lang::Result<Self> {
                let from: AccountInfo = anchor_lang::Accounts::try_accounts(
                        __program_id,
                        __accounts,
                        __ix_data,
                        __bumps,
                        __reallocs,
                    )
                    .map_err(|e| e.with_account_name("from"))?;
                let to: AccountInfo = anchor_lang::Accounts::try_accounts(
                        __program_id,
                        __accounts,
                        __ix_data,
                        __bumps,
                        __reallocs,
                    )
                    .map_err(|e| e.with_account_name("to"))?;
                let base: AccountInfo = anchor_lang::Accounts::try_accounts(
                        __program_id,
                        __accounts,
                        __ix_data,
                        __bumps,
                        __reallocs,
                    )
                    .map_err(|e| e.with_account_name("base"))?;
                let system_program: anchor_lang::accounts::program::Program<System> = anchor_lang::Accounts::try_accounts(
                        __program_id,
                        __accounts,
                        __ix_data,
                        __bumps,
                        __reallocs,
                    )
                    .map_err(|e| e.with_account_name("system_program"))?;
                let program: AccountInfo = anchor_lang::Accounts::try_accounts(
                        __program_id,
                        __accounts,
                        __ix_data,
                        __bumps,
                        __reallocs,
                    )
                    .map_err(|e| e.with_account_name("program"))?;
                if !&from.is_signer {
                    return Err(
                        anchor_lang::error::Error::from(
                                anchor_lang::error::ErrorCode::ConstraintSigner,
                            )
                            .with_account_name("from"),
                    );
                }
                if !&to.is_writable {
                    return Err(
                        anchor_lang::error::Error::from(
                                anchor_lang::error::ErrorCode::ConstraintMut,
                            )
                            .with_account_name("to"),
                    );
                }
                let (__pda_address, __bump) = Pubkey::find_program_address(
                    &[],
                    &__program_id,
                );
                __bumps.base = __bump;
                if base.key() != __pda_address {
                    return Err(
                        anchor_lang::error::Error::from(
                                anchor_lang::error::ErrorCode::ConstraintSeeds,
                            )
                            .with_account_name("base")
                            .with_pubkeys((base.key(), __pda_address)),
                    );
                }
                if !&program.executable {
                    return Err(
                        anchor_lang::error::Error::from(
                                anchor_lang::error::ErrorCode::ConstraintExecutable,
                            )
                            .with_account_name("program"),
                    );
                }
                Ok(IdlCreateAccounts {
                    from,
                    to,
                    base,
                    system_program,
                    program,
                })
            }
        }
        #[automatically_derived]
        impl<'info> anchor_lang::ToAccountInfos<'info> for IdlCreateAccounts<'info>
        where
            'info: 'info,
        {
            fn to_account_infos(
                &self,
            ) -> Vec<anchor_lang::solana_program::account_info::AccountInfo<'info>> {
                let mut account_infos = ::alloc::vec::Vec::new();
                account_infos.extend(self.from.to_account_infos());
                account_infos.extend(self.to.to_account_infos());
                account_infos.extend(self.base.to_account_infos());
                account_infos.extend(self.system_program.to_account_infos());
                account_infos.extend(self.program.to_account_infos());
                account_infos
            }
        }
        #[automatically_derived]
        impl<'info> anchor_lang::ToAccountMetas for IdlCreateAccounts<'info> {
            fn to_account_metas(
                &self,
                is_signer: Option<bool>,
            ) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
                let mut account_metas = ::alloc::vec::Vec::new();
                account_metas.extend(self.from.to_account_metas(Some(true)));
                account_metas.extend(self.to.to_account_metas(None));
                account_metas.extend(self.base.to_account_metas(None));
                account_metas.extend(self.system_program.to_account_metas(None));
                account_metas.extend(self.program.to_account_metas(None));
                account_metas
            }
        }
        #[automatically_derived]
        impl<'info> anchor_lang::AccountsExit<'info> for IdlCreateAccounts<'info>
        where
            'info: 'info,
        {
            fn exit(
                &self,
                program_id: &anchor_lang::solana_program::pubkey::Pubkey,
            ) -> anchor_lang::Result<()> {
                anchor_lang::AccountsExit::exit(&self.to, program_id)
                    .map_err(|e| e.with_account_name("to"))?;
                Ok(())
            }
        }
        pub struct IdlCreateAccountsBumps {
            pub base: u8,
        }
        #[automatically_derived]
        impl ::core::fmt::Debug for IdlCreateAccountsBumps {
            #[inline]
            fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                ::core::fmt::Formatter::debug_struct_field1_finish(
                    f,
                    "IdlCreateAccountsBumps",
                    "base",
                    &&self.base,
                )
            }
        }
        impl Default for IdlCreateAccountsBumps {
            fn default() -> Self {
                IdlCreateAccountsBumps {
                    base: u8::MAX,
                }
            }
        }
        impl<'info> anchor_lang::Bumps for IdlCreateAccounts<'info>
        where
            'info: 'info,
        {
            type Bumps = IdlCreateAccountsBumps;
        }
        /// An internal, Anchor generated module. This is used (as an
        /// implementation detail), to generate a struct for a given
        /// `#[derive(Accounts)]` implementation, where each field is a Pubkey,
        /// instead of an `AccountInfo`. This is useful for clients that want
        /// to generate a list of accounts, without explicitly knowing the
        /// order all the fields should be in.
        ///
        /// To access the struct in this module, one should use the sibling
        /// `accounts` module (also generated), which re-exports this.
        pub(crate) mod __client_accounts_idl_create_accounts {
            use super::*;
            use anchor_lang::prelude::borsh;
            /// Generated client accounts for [`IdlCreateAccounts`].
            pub struct IdlCreateAccounts {
                pub from: Pubkey,
                pub to: Pubkey,
                pub base: Pubkey,
                pub system_program: Pubkey,
                pub program: Pubkey,
            }
            impl borsh::ser::BorshSerialize for IdlCreateAccounts
            where
                Pubkey: borsh::ser::BorshSerialize,
                Pubkey: borsh::ser::BorshSerialize,
                Pubkey: borsh::ser::BorshSerialize,
                Pubkey: borsh::ser::BorshSerialize,
                Pubkey: borsh::ser::BorshSerialize,
            {
                fn serialize<W: borsh::maybestd::io::Write>(
                    &self,
                    writer: &mut W,
                ) -> ::core::result::Result<(), borsh::maybestd::io::Error> {
                    borsh::BorshSerialize::serialize(&self.from, writer)?;
                    borsh::BorshSerialize::serialize(&self.to, writer)?;
                    borsh::BorshSerialize::serialize(&self.base, writer)?;
                    borsh::BorshSerialize::serialize(&self.system_program, writer)?;
                    borsh::BorshSerialize::serialize(&self.program, writer)?;
                    Ok(())
                }
            }
            #[automatically_derived]
            impl anchor_lang::ToAccountMetas for IdlCreateAccounts {
                fn to_account_metas(
                    &self,
                    is_signer: Option<bool>,
                ) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
                    let mut account_metas = ::alloc::vec::Vec::new();
                    account_metas
                        .push(
                            anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                                self.from,
                                true,
                            ),
                        );
                    account_metas
                        .push(
                            anchor_lang::solana_program::instruction::AccountMeta::new(
                                self.to,
                                false,
                            ),
                        );
                    account_metas
                        .push(
                            anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                                self.base,
                                false,
                            ),
                        );
                    account_metas
                        .push(
                            anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                                self.system_program,
                                false,
                            ),
                        );
                    account_metas
                        .push(
                            anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                                self.program,
                                false,
                            ),
                        );
                    account_metas
                }
            }
        }
        /// An internal, Anchor generated module. This is used (as an
        /// implementation detail), to generate a CPI struct for a given
        /// `#[derive(Accounts)]` implementation, where each field is an
        /// AccountInfo.
        ///
        /// To access the struct in this module, one should use the sibling
        /// [`cpi::accounts`] module (also generated), which re-exports this.
        pub(crate) mod __cpi_client_accounts_idl_create_accounts {
            use super::*;
            /// Generated CPI struct of the accounts for [`IdlCreateAccounts`].
            pub struct IdlCreateAccounts<'info> {
                pub from: anchor_lang::solana_program::account_info::AccountInfo<'info>,
                pub to: anchor_lang::solana_program::account_info::AccountInfo<'info>,
                pub base: anchor_lang::solana_program::account_info::AccountInfo<'info>,
                pub system_program: anchor_lang::solana_program::account_info::AccountInfo<
                    'info,
                >,
                pub program: anchor_lang::solana_program::account_info::AccountInfo<
                    'info,
                >,
            }
            #[automatically_derived]
            impl<'info> anchor_lang::ToAccountMetas for IdlCreateAccounts<'info> {
                fn to_account_metas(
                    &self,
                    is_signer: Option<bool>,
                ) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
                    let mut account_metas = ::alloc::vec::Vec::new();
                    account_metas
                        .push(
                            anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                                anchor_lang::Key::key(&self.from),
                                true,
                            ),
                        );
                    account_metas
                        .push(
                            anchor_lang::solana_program::instruction::AccountMeta::new(
                                anchor_lang::Key::key(&self.to),
                                false,
                            ),
                        );
                    account_metas
                        .push(
                            anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                                anchor_lang::Key::key(&self.base),
                                false,
                            ),
                        );
                    account_metas
                        .push(
                            anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                                anchor_lang::Key::key(&self.system_program),
                                false,
                            ),
                        );
                    account_metas
                        .push(
                            anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                                anchor_lang::Key::key(&self.program),
                                false,
                            ),
                        );
                    account_metas
                }
            }
            #[automatically_derived]
            impl<'info> anchor_lang::ToAccountInfos<'info> for IdlCreateAccounts<'info> {
                fn to_account_infos(
                    &self,
                ) -> Vec<anchor_lang::solana_program::account_info::AccountInfo<'info>> {
                    let mut account_infos = ::alloc::vec::Vec::new();
                    account_infos
                        .extend(
                            anchor_lang::ToAccountInfos::to_account_infos(&self.from),
                        );
                    account_infos
                        .extend(anchor_lang::ToAccountInfos::to_account_infos(&self.to));
                    account_infos
                        .extend(
                            anchor_lang::ToAccountInfos::to_account_infos(&self.base),
                        );
                    account_infos
                        .extend(
                            anchor_lang::ToAccountInfos::to_account_infos(
                                &self.system_program,
                            ),
                        );
                    account_infos
                        .extend(
                            anchor_lang::ToAccountInfos::to_account_infos(&self.program),
                        );
                    account_infos
                }
            }
        }
        pub struct IdlAccounts<'info> {
            #[account(mut, has_one = authority)]
            pub idl: Account<'info, IdlAccount>,
            #[account(constraint = authority.key!= &ERASED_AUTHORITY)]
            pub authority: Signer<'info>,
        }
        #[automatically_derived]
        impl<'info> anchor_lang::Accounts<'info, IdlAccountsBumps> for IdlAccounts<'info>
        where
            'info: 'info,
        {
            #[inline(never)]
            fn try_accounts(
                __program_id: &anchor_lang::solana_program::pubkey::Pubkey,
                __accounts: &mut &'info [anchor_lang::solana_program::account_info::AccountInfo<
                    'info,
                >],
                __ix_data: &[u8],
                __bumps: &mut IdlAccountsBumps,
                __reallocs: &mut std::collections::BTreeSet<
                    anchor_lang::solana_program::pubkey::Pubkey,
                >,
            ) -> anchor_lang::Result<Self> {
                let idl: anchor_lang::accounts::account::Account<IdlAccount> = anchor_lang::Accounts::try_accounts(
                        __program_id,
                        __accounts,
                        __ix_data,
                        __bumps,
                        __reallocs,
                    )
                    .map_err(|e| e.with_account_name("idl"))?;
                let authority: Signer = anchor_lang::Accounts::try_accounts(
                        __program_id,
                        __accounts,
                        __ix_data,
                        __bumps,
                        __reallocs,
                    )
                    .map_err(|e| e.with_account_name("authority"))?;
                if !AsRef::<AccountInfo>::as_ref(&idl).is_writable {
                    return Err(
                        anchor_lang::error::Error::from(
                                anchor_lang::error::ErrorCode::ConstraintMut,
                            )
                            .with_account_name("idl"),
                    );
                }
                {
                    let my_key = idl.authority;
                    let target_key = authority.key();
                    if my_key != target_key {
                        return Err(
                            anchor_lang::error::Error::from(
                                    anchor_lang::error::ErrorCode::ConstraintHasOne,
                                )
                                .with_account_name("idl")
                                .with_pubkeys((my_key, target_key)),
                        );
                    }
                }
                if !(authority.key != &ERASED_AUTHORITY) {
                    return Err(
                        anchor_lang::error::Error::from(
                                anchor_lang::error::ErrorCode::ConstraintRaw,
                            )
                            .with_account_name("authority"),
                    );
                }
                Ok(IdlAccounts { idl, authority })
            }
        }
        #[automatically_derived]
        impl<'info> anchor_lang::ToAccountInfos<'info> for IdlAccounts<'info>
        where
            'info: 'info,
        {
            fn to_account_infos(
                &self,
            ) -> Vec<anchor_lang::solana_program::account_info::AccountInfo<'info>> {
                let mut account_infos = ::alloc::vec::Vec::new();
                account_infos.extend(self.idl.to_account_infos());
                account_infos.extend(self.authority.to_account_infos());
                account_infos
            }
        }
        #[automatically_derived]
        impl<'info> anchor_lang::ToAccountMetas for IdlAccounts<'info> {
            fn to_account_metas(
                &self,
                is_signer: Option<bool>,
            ) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
                let mut account_metas = ::alloc::vec::Vec::new();
                account_metas.extend(self.idl.to_account_metas(None));
                account_metas.extend(self.authority.to_account_metas(None));
                account_metas
            }
        }
        #[automatically_derived]
        impl<'info> anchor_lang::AccountsExit<'info> for IdlAccounts<'info>
        where
            'info: 'info,
        {
            fn exit(
                &self,
                program_id: &anchor_lang::solana_program::pubkey::Pubkey,
            ) -> anchor_lang::Result<()> {
                anchor_lang::AccountsExit::exit(&self.idl, program_id)
                    .map_err(|e| e.with_account_name("idl"))?;
                Ok(())
            }
        }
        pub struct IdlAccountsBumps {}
        #[automatically_derived]
        impl ::core::fmt::Debug for IdlAccountsBumps {
            #[inline]
            fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                ::core::fmt::Formatter::write_str(f, "IdlAccountsBumps")
            }
        }
        impl Default for IdlAccountsBumps {
            fn default() -> Self {
                IdlAccountsBumps {}
            }
        }
        impl<'info> anchor_lang::Bumps for IdlAccounts<'info>
        where
            'info: 'info,
        {
            type Bumps = IdlAccountsBumps;
        }
        /// An internal, Anchor generated module. This is used (as an
        /// implementation detail), to generate a struct for a given
        /// `#[derive(Accounts)]` implementation, where each field is a Pubkey,
        /// instead of an `AccountInfo`. This is useful for clients that want
        /// to generate a list of accounts, without explicitly knowing the
        /// order all the fields should be in.
        ///
        /// To access the struct in this module, one should use the sibling
        /// `accounts` module (also generated), which re-exports this.
        pub(crate) mod __client_accounts_idl_accounts {
            use super::*;
            use anchor_lang::prelude::borsh;
            /// Generated client accounts for [`IdlAccounts`].
            pub struct IdlAccounts {
                pub idl: Pubkey,
                pub authority: Pubkey,
            }
            impl borsh::ser::BorshSerialize for IdlAccounts
            where
                Pubkey: borsh::ser::BorshSerialize,
                Pubkey: borsh::ser::BorshSerialize,
            {
                fn serialize<W: borsh::maybestd::io::Write>(
                    &self,
                    writer: &mut W,
                ) -> ::core::result::Result<(), borsh::maybestd::io::Error> {
                    borsh::BorshSerialize::serialize(&self.idl, writer)?;
                    borsh::BorshSerialize::serialize(&self.authority, writer)?;
                    Ok(())
                }
            }
            #[automatically_derived]
            impl anchor_lang::ToAccountMetas for IdlAccounts {
                fn to_account_metas(
                    &self,
                    is_signer: Option<bool>,
                ) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
                    let mut account_metas = ::alloc::vec::Vec::new();
                    account_metas
                        .push(
                            anchor_lang::solana_program::instruction::AccountMeta::new(
                                self.idl,
                                false,
                            ),
                        );
                    account_metas
                        .push(
                            anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                                self.authority,
                                true,
                            ),
                        );
                    account_metas
                }
            }
        }
        /// An internal, Anchor generated module. This is used (as an
        /// implementation detail), to generate a CPI struct for a given
        /// `#[derive(Accounts)]` implementation, where each field is an
        /// AccountInfo.
        ///
        /// To access the struct in this module, one should use the sibling
        /// [`cpi::accounts`] module (also generated), which re-exports this.
        pub(crate) mod __cpi_client_accounts_idl_accounts {
            use super::*;
            /// Generated CPI struct of the accounts for [`IdlAccounts`].
            pub struct IdlAccounts<'info> {
                pub idl: anchor_lang::solana_program::account_info::AccountInfo<'info>,
                pub authority: anchor_lang::solana_program::account_info::AccountInfo<
                    'info,
                >,
            }
            #[automatically_derived]
            impl<'info> anchor_lang::ToAccountMetas for IdlAccounts<'info> {
                fn to_account_metas(
                    &self,
                    is_signer: Option<bool>,
                ) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
                    let mut account_metas = ::alloc::vec::Vec::new();
                    account_metas
                        .push(
                            anchor_lang::solana_program::instruction::AccountMeta::new(
                                anchor_lang::Key::key(&self.idl),
                                false,
                            ),
                        );
                    account_metas
                        .push(
                            anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                                anchor_lang::Key::key(&self.authority),
                                true,
                            ),
                        );
                    account_metas
                }
            }
            #[automatically_derived]
            impl<'info> anchor_lang::ToAccountInfos<'info> for IdlAccounts<'info> {
                fn to_account_infos(
                    &self,
                ) -> Vec<anchor_lang::solana_program::account_info::AccountInfo<'info>> {
                    let mut account_infos = ::alloc::vec::Vec::new();
                    account_infos
                        .extend(
                            anchor_lang::ToAccountInfos::to_account_infos(&self.idl),
                        );
                    account_infos
                        .extend(
                            anchor_lang::ToAccountInfos::to_account_infos(
                                &self.authority,
                            ),
                        );
                    account_infos
                }
            }
        }
        pub struct IdlResizeAccount<'info> {
            #[account(mut, has_one = authority)]
            pub idl: Account<'info, IdlAccount>,
            #[account(mut, constraint = authority.key!= &ERASED_AUTHORITY)]
            pub authority: Signer<'info>,
            pub system_program: Program<'info, System>,
        }
        #[automatically_derived]
        impl<'info> anchor_lang::Accounts<'info, IdlResizeAccountBumps>
        for IdlResizeAccount<'info>
        where
            'info: 'info,
        {
            #[inline(never)]
            fn try_accounts(
                __program_id: &anchor_lang::solana_program::pubkey::Pubkey,
                __accounts: &mut &'info [anchor_lang::solana_program::account_info::AccountInfo<
                    'info,
                >],
                __ix_data: &[u8],
                __bumps: &mut IdlResizeAccountBumps,
                __reallocs: &mut std::collections::BTreeSet<
                    anchor_lang::solana_program::pubkey::Pubkey,
                >,
            ) -> anchor_lang::Result<Self> {
                let idl: anchor_lang::accounts::account::Account<IdlAccount> = anchor_lang::Accounts::try_accounts(
                        __program_id,
                        __accounts,
                        __ix_data,
                        __bumps,
                        __reallocs,
                    )
                    .map_err(|e| e.with_account_name("idl"))?;
                let authority: Signer = anchor_lang::Accounts::try_accounts(
                        __program_id,
                        __accounts,
                        __ix_data,
                        __bumps,
                        __reallocs,
                    )
                    .map_err(|e| e.with_account_name("authority"))?;
                let system_program: anchor_lang::accounts::program::Program<System> = anchor_lang::Accounts::try_accounts(
                        __program_id,
                        __accounts,
                        __ix_data,
                        __bumps,
                        __reallocs,
                    )
                    .map_err(|e| e.with_account_name("system_program"))?;
                if !AsRef::<AccountInfo>::as_ref(&idl).is_writable {
                    return Err(
                        anchor_lang::error::Error::from(
                                anchor_lang::error::ErrorCode::ConstraintMut,
                            )
                            .with_account_name("idl"),
                    );
                }
                {
                    let my_key = idl.authority;
                    let target_key = authority.key();
                    if my_key != target_key {
                        return Err(
                            anchor_lang::error::Error::from(
                                    anchor_lang::error::ErrorCode::ConstraintHasOne,
                                )
                                .with_account_name("idl")
                                .with_pubkeys((my_key, target_key)),
                        );
                    }
                }
                if !AsRef::<AccountInfo>::as_ref(&authority).is_writable {
                    return Err(
                        anchor_lang::error::Error::from(
                                anchor_lang::error::ErrorCode::ConstraintMut,
                            )
                            .with_account_name("authority"),
                    );
                }
                if !(authority.key != &ERASED_AUTHORITY) {
                    return Err(
                        anchor_lang::error::Error::from(
                                anchor_lang::error::ErrorCode::ConstraintRaw,
                            )
                            .with_account_name("authority"),
                    );
                }
                Ok(IdlResizeAccount {
                    idl,
                    authority,
                    system_program,
                })
            }
        }
        #[automatically_derived]
        impl<'info> anchor_lang::ToAccountInfos<'info> for IdlResizeAccount<'info>
        where
            'info: 'info,
        {
            fn to_account_infos(
                &self,
            ) -> Vec<anchor_lang::solana_program::account_info::AccountInfo<'info>> {
                let mut account_infos = ::alloc::vec::Vec::new();
                account_infos.extend(self.idl.to_account_infos());
                account_infos.extend(self.authority.to_account_infos());
                account_infos.extend(self.system_program.to_account_infos());
                account_infos
            }
        }
        #[automatically_derived]
        impl<'info> anchor_lang::ToAccountMetas for IdlResizeAccount<'info> {
            fn to_account_metas(
                &self,
                is_signer: Option<bool>,
            ) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
                let mut account_metas = ::alloc::vec::Vec::new();
                account_metas.extend(self.idl.to_account_metas(None));
                account_metas.extend(self.authority.to_account_metas(None));
                account_metas.extend(self.system_program.to_account_metas(None));
                account_metas
            }
        }
        #[automatically_derived]
        impl<'info> anchor_lang::AccountsExit<'info> for IdlResizeAccount<'info>
        where
            'info: 'info,
        {
            fn exit(
                &self,
                program_id: &anchor_lang::solana_program::pubkey::Pubkey,
            ) -> anchor_lang::Result<()> {
                anchor_lang::AccountsExit::exit(&self.idl, program_id)
                    .map_err(|e| e.with_account_name("idl"))?;
                anchor_lang::AccountsExit::exit(&self.authority, program_id)
                    .map_err(|e| e.with_account_name("authority"))?;
                Ok(())
            }
        }
        pub struct IdlResizeAccountBumps {}
        #[automatically_derived]
        impl ::core::fmt::Debug for IdlResizeAccountBumps {
            #[inline]
            fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                ::core::fmt::Formatter::write_str(f, "IdlResizeAccountBumps")
            }
        }
        impl Default for IdlResizeAccountBumps {
            fn default() -> Self {
                IdlResizeAccountBumps {}
            }
        }
        impl<'info> anchor_lang::Bumps for IdlResizeAccount<'info>
        where
            'info: 'info,
        {
            type Bumps = IdlResizeAccountBumps;
        }
        /// An internal, Anchor generated module. This is used (as an
        /// implementation detail), to generate a struct for a given
        /// `#[derive(Accounts)]` implementation, where each field is a Pubkey,
        /// instead of an `AccountInfo`. This is useful for clients that want
        /// to generate a list of accounts, without explicitly knowing the
        /// order all the fields should be in.
        ///
        /// To access the struct in this module, one should use the sibling
        /// `accounts` module (also generated), which re-exports this.
        pub(crate) mod __client_accounts_idl_resize_account {
            use super::*;
            use anchor_lang::prelude::borsh;
            /// Generated client accounts for [`IdlResizeAccount`].
            pub struct IdlResizeAccount {
                pub idl: Pubkey,
                pub authority: Pubkey,
                pub system_program: Pubkey,
            }
            impl borsh::ser::BorshSerialize for IdlResizeAccount
            where
                Pubkey: borsh::ser::BorshSerialize,
                Pubkey: borsh::ser::BorshSerialize,
                Pubkey: borsh::ser::BorshSerialize,
            {
                fn serialize<W: borsh::maybestd::io::Write>(
                    &self,
                    writer: &mut W,
                ) -> ::core::result::Result<(), borsh::maybestd::io::Error> {
                    borsh::BorshSerialize::serialize(&self.idl, writer)?;
                    borsh::BorshSerialize::serialize(&self.authority, writer)?;
                    borsh::BorshSerialize::serialize(&self.system_program, writer)?;
                    Ok(())
                }
            }
            #[automatically_derived]
            impl anchor_lang::ToAccountMetas for IdlResizeAccount {
                fn to_account_metas(
                    &self,
                    is_signer: Option<bool>,
                ) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
                    let mut account_metas = ::alloc::vec::Vec::new();
                    account_metas
                        .push(
                            anchor_lang::solana_program::instruction::AccountMeta::new(
                                self.idl,
                                false,
                            ),
                        );
                    account_metas
                        .push(
                            anchor_lang::solana_program::instruction::AccountMeta::new(
                                self.authority,
                                true,
                            ),
                        );
                    account_metas
                        .push(
                            anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                                self.system_program,
                                false,
                            ),
                        );
                    account_metas
                }
            }
        }
        /// An internal, Anchor generated module. This is used (as an
        /// implementation detail), to generate a CPI struct for a given
        /// `#[derive(Accounts)]` implementation, where each field is an
        /// AccountInfo.
        ///
        /// To access the struct in this module, one should use the sibling
        /// [`cpi::accounts`] module (also generated), which re-exports this.
        pub(crate) mod __cpi_client_accounts_idl_resize_account {
            use super::*;
            /// Generated CPI struct of the accounts for [`IdlResizeAccount`].
            pub struct IdlResizeAccount<'info> {
                pub idl: anchor_lang::solana_program::account_info::AccountInfo<'info>,
                pub authority: anchor_lang::solana_program::account_info::AccountInfo<
                    'info,
                >,
                pub system_program: anchor_lang::solana_program::account_info::AccountInfo<
                    'info,
                >,
            }
            #[automatically_derived]
            impl<'info> anchor_lang::ToAccountMetas for IdlResizeAccount<'info> {
                fn to_account_metas(
                    &self,
                    is_signer: Option<bool>,
                ) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
                    let mut account_metas = ::alloc::vec::Vec::new();
                    account_metas
                        .push(
                            anchor_lang::solana_program::instruction::AccountMeta::new(
                                anchor_lang::Key::key(&self.idl),
                                false,
                            ),
                        );
                    account_metas
                        .push(
                            anchor_lang::solana_program::instruction::AccountMeta::new(
                                anchor_lang::Key::key(&self.authority),
                                true,
                            ),
                        );
                    account_metas
                        .push(
                            anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                                anchor_lang::Key::key(&self.system_program),
                                false,
                            ),
                        );
                    account_metas
                }
            }
            #[automatically_derived]
            impl<'info> anchor_lang::ToAccountInfos<'info> for IdlResizeAccount<'info> {
                fn to_account_infos(
                    &self,
                ) -> Vec<anchor_lang::solana_program::account_info::AccountInfo<'info>> {
                    let mut account_infos = ::alloc::vec::Vec::new();
                    account_infos
                        .extend(
                            anchor_lang::ToAccountInfos::to_account_infos(&self.idl),
                        );
                    account_infos
                        .extend(
                            anchor_lang::ToAccountInfos::to_account_infos(
                                &self.authority,
                            ),
                        );
                    account_infos
                        .extend(
                            anchor_lang::ToAccountInfos::to_account_infos(
                                &self.system_program,
                            ),
                        );
                    account_infos
                }
            }
        }
        pub struct IdlCreateBuffer<'info> {
            #[account(zero)]
            pub buffer: Account<'info, IdlAccount>,
            #[account(constraint = authority.key!= &ERASED_AUTHORITY)]
            pub authority: Signer<'info>,
        }
        #[automatically_derived]
        impl<'info> anchor_lang::Accounts<'info, IdlCreateBufferBumps>
        for IdlCreateBuffer<'info>
        where
            'info: 'info,
        {
            #[inline(never)]
            fn try_accounts(
                __program_id: &anchor_lang::solana_program::pubkey::Pubkey,
                __accounts: &mut &'info [anchor_lang::solana_program::account_info::AccountInfo<
                    'info,
                >],
                __ix_data: &[u8],
                __bumps: &mut IdlCreateBufferBumps,
                __reallocs: &mut std::collections::BTreeSet<
                    anchor_lang::solana_program::pubkey::Pubkey,
                >,
            ) -> anchor_lang::Result<Self> {
                if __accounts.is_empty() {
                    return Err(
                        anchor_lang::error::ErrorCode::AccountNotEnoughKeys.into(),
                    );
                }
                let buffer = &__accounts[0];
                *__accounts = &__accounts[1..];
                let authority: Signer = anchor_lang::Accounts::try_accounts(
                        __program_id,
                        __accounts,
                        __ix_data,
                        __bumps,
                        __reallocs,
                    )
                    .map_err(|e| e.with_account_name("authority"))?;
                let __anchor_rent = Rent::get()?;
                let buffer: anchor_lang::accounts::account::Account<IdlAccount> = {
                    let mut __data: &[u8] = &buffer.try_borrow_data()?;
                    let __disc = &__data[..IdlAccount::DISCRIMINATOR.len()];
                    let __has_disc = __disc.iter().any(|b| *b != 0);
                    if __has_disc {
                        return Err(
                            anchor_lang::error::Error::from(
                                    anchor_lang::error::ErrorCode::ConstraintZero,
                                )
                                .with_account_name("buffer"),
                        );
                    }
                    match anchor_lang::accounts::account::Account::try_from_unchecked(
                        &buffer,
                    ) {
                        Ok(val) => val,
                        Err(e) => return Err(e.with_account_name("buffer")),
                    }
                };
                if !AsRef::<AccountInfo>::as_ref(&buffer).is_writable {
                    return Err(
                        anchor_lang::error::Error::from(
                                anchor_lang::error::ErrorCode::ConstraintMut,
                            )
                            .with_account_name("buffer"),
                    );
                }
                if !__anchor_rent
                    .is_exempt(
                        buffer.to_account_info().lamports(),
                        buffer.to_account_info().try_data_len()?,
                    )
                {
                    return Err(
                        anchor_lang::error::Error::from(
                                anchor_lang::error::ErrorCode::ConstraintRentExempt,
                            )
                            .with_account_name("buffer"),
                    );
                }
                if !(authority.key != &ERASED_AUTHORITY) {
                    return Err(
                        anchor_lang::error::Error::from(
                                anchor_lang::error::ErrorCode::ConstraintRaw,
                            )
                            .with_account_name("authority"),
                    );
                }
                Ok(IdlCreateBuffer {
                    buffer,
                    authority,
                })
            }
        }
        #[automatically_derived]
        impl<'info> anchor_lang::ToAccountInfos<'info> for IdlCreateBuffer<'info>
        where
            'info: 'info,
        {
            fn to_account_infos(
                &self,
            ) -> Vec<anchor_lang::solana_program::account_info::AccountInfo<'info>> {
                let mut account_infos = ::alloc::vec::Vec::new();
                account_infos.extend(self.buffer.to_account_infos());
                account_infos.extend(self.authority.to_account_infos());
                account_infos
            }
        }
        #[automatically_derived]
        impl<'info> anchor_lang::ToAccountMetas for IdlCreateBuffer<'info> {
            fn to_account_metas(
                &self,
                is_signer: Option<bool>,
            ) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
                let mut account_metas = ::alloc::vec::Vec::new();
                account_metas.extend(self.buffer.to_account_metas(None));
                account_metas.extend(self.authority.to_account_metas(None));
                account_metas
            }
        }
        #[automatically_derived]
        impl<'info> anchor_lang::AccountsExit<'info> for IdlCreateBuffer<'info>
        where
            'info: 'info,
        {
            fn exit(
                &self,
                program_id: &anchor_lang::solana_program::pubkey::Pubkey,
            ) -> anchor_lang::Result<()> {
                anchor_lang::AccountsExit::exit(&self.buffer, program_id)
                    .map_err(|e| e.with_account_name("buffer"))?;
                Ok(())
            }
        }
        pub struct IdlCreateBufferBumps {}
        #[automatically_derived]
        impl ::core::fmt::Debug for IdlCreateBufferBumps {
            #[inline]
            fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                ::core::fmt::Formatter::write_str(f, "IdlCreateBufferBumps")
            }
        }
        impl Default for IdlCreateBufferBumps {
            fn default() -> Self {
                IdlCreateBufferBumps {}
            }
        }
        impl<'info> anchor_lang::Bumps for IdlCreateBuffer<'info>
        where
            'info: 'info,
        {
            type Bumps = IdlCreateBufferBumps;
        }
        /// An internal, Anchor generated module. This is used (as an
        /// implementation detail), to generate a struct for a given
        /// `#[derive(Accounts)]` implementation, where each field is a Pubkey,
        /// instead of an `AccountInfo`. This is useful for clients that want
        /// to generate a list of accounts, without explicitly knowing the
        /// order all the fields should be in.
        ///
        /// To access the struct in this module, one should use the sibling
        /// `accounts` module (also generated), which re-exports this.
        pub(crate) mod __client_accounts_idl_create_buffer {
            use super::*;
            use anchor_lang::prelude::borsh;
            /// Generated client accounts for [`IdlCreateBuffer`].
            pub struct IdlCreateBuffer {
                pub buffer: Pubkey,
                pub authority: Pubkey,
            }
            impl borsh::ser::BorshSerialize for IdlCreateBuffer
            where
                Pubkey: borsh::ser::BorshSerialize,
                Pubkey: borsh::ser::BorshSerialize,
            {
                fn serialize<W: borsh::maybestd::io::Write>(
                    &self,
                    writer: &mut W,
                ) -> ::core::result::Result<(), borsh::maybestd::io::Error> {
                    borsh::BorshSerialize::serialize(&self.buffer, writer)?;
                    borsh::BorshSerialize::serialize(&self.authority, writer)?;
                    Ok(())
                }
            }
            #[automatically_derived]
            impl anchor_lang::ToAccountMetas for IdlCreateBuffer {
                fn to_account_metas(
                    &self,
                    is_signer: Option<bool>,
                ) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
                    let mut account_metas = ::alloc::vec::Vec::new();
                    account_metas
                        .push(
                            anchor_lang::solana_program::instruction::AccountMeta::new(
                                self.buffer,
                                false,
                            ),
                        );
                    account_metas
                        .push(
                            anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                                self.authority,
                                true,
                            ),
                        );
                    account_metas
                }
            }
        }
        /// An internal, Anchor generated module. This is used (as an
        /// implementation detail), to generate a CPI struct for a given
        /// `#[derive(Accounts)]` implementation, where each field is an
        /// AccountInfo.
        ///
        /// To access the struct in this module, one should use the sibling
        /// [`cpi::accounts`] module (also generated), which re-exports this.
        pub(crate) mod __cpi_client_accounts_idl_create_buffer {
            use super::*;
            /// Generated CPI struct of the accounts for [`IdlCreateBuffer`].
            pub struct IdlCreateBuffer<'info> {
                pub buffer: anchor_lang::solana_program::account_info::AccountInfo<
                    'info,
                >,
                pub authority: anchor_lang::solana_program::account_info::AccountInfo<
                    'info,
                >,
            }
            #[automatically_derived]
            impl<'info> anchor_lang::ToAccountMetas for IdlCreateBuffer<'info> {
                fn to_account_metas(
                    &self,
                    is_signer: Option<bool>,
                ) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
                    let mut account_metas = ::alloc::vec::Vec::new();
                    account_metas
                        .push(
                            anchor_lang::solana_program::instruction::AccountMeta::new(
                                anchor_lang::Key::key(&self.buffer),
                                false,
                            ),
                        );
                    account_metas
                        .push(
                            anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                                anchor_lang::Key::key(&self.authority),
                                true,
                            ),
                        );
                    account_metas
                }
            }
            #[automatically_derived]
            impl<'info> anchor_lang::ToAccountInfos<'info> for IdlCreateBuffer<'info> {
                fn to_account_infos(
                    &self,
                ) -> Vec<anchor_lang::solana_program::account_info::AccountInfo<'info>> {
                    let mut account_infos = ::alloc::vec::Vec::new();
                    account_infos
                        .extend(
                            anchor_lang::ToAccountInfos::to_account_infos(&self.buffer),
                        );
                    account_infos
                        .extend(
                            anchor_lang::ToAccountInfos::to_account_infos(
                                &self.authority,
                            ),
                        );
                    account_infos
                }
            }
        }
        pub struct IdlSetBuffer<'info> {
            #[account(mut, constraint = buffer.authority = = idl.authority)]
            pub buffer: Account<'info, IdlAccount>,
            #[account(mut, has_one = authority)]
            pub idl: Account<'info, IdlAccount>,
            #[account(constraint = authority.key!= &ERASED_AUTHORITY)]
            pub authority: Signer<'info>,
        }
        #[automatically_derived]
        impl<'info> anchor_lang::Accounts<'info, IdlSetBufferBumps>
        for IdlSetBuffer<'info>
        where
            'info: 'info,
        {
            #[inline(never)]
            fn try_accounts(
                __program_id: &anchor_lang::solana_program::pubkey::Pubkey,
                __accounts: &mut &'info [anchor_lang::solana_program::account_info::AccountInfo<
                    'info,
                >],
                __ix_data: &[u8],
                __bumps: &mut IdlSetBufferBumps,
                __reallocs: &mut std::collections::BTreeSet<
                    anchor_lang::solana_program::pubkey::Pubkey,
                >,
            ) -> anchor_lang::Result<Self> {
                let buffer: anchor_lang::accounts::account::Account<IdlAccount> = anchor_lang::Accounts::try_accounts(
                        __program_id,
                        __accounts,
                        __ix_data,
                        __bumps,
                        __reallocs,
                    )
                    .map_err(|e| e.with_account_name("buffer"))?;
                let idl: anchor_lang::accounts::account::Account<IdlAccount> = anchor_lang::Accounts::try_accounts(
                        __program_id,
                        __accounts,
                        __ix_data,
                        __bumps,
                        __reallocs,
                    )
                    .map_err(|e| e.with_account_name("idl"))?;
                let authority: Signer = anchor_lang::Accounts::try_accounts(
                        __program_id,
                        __accounts,
                        __ix_data,
                        __bumps,
                        __reallocs,
                    )
                    .map_err(|e| e.with_account_name("authority"))?;
                if !AsRef::<AccountInfo>::as_ref(&buffer).is_writable {
                    return Err(
                        anchor_lang::error::Error::from(
                                anchor_lang::error::ErrorCode::ConstraintMut,
                            )
                            .with_account_name("buffer"),
                    );
                }
                if !(buffer.authority == idl.authority) {
                    return Err(
                        anchor_lang::error::Error::from(
                                anchor_lang::error::ErrorCode::ConstraintRaw,
                            )
                            .with_account_name("buffer"),
                    );
                }
                if !AsRef::<AccountInfo>::as_ref(&idl).is_writable {
                    return Err(
                        anchor_lang::error::Error::from(
                                anchor_lang::error::ErrorCode::ConstraintMut,
                            )
                            .with_account_name("idl"),
                    );
                }
                {
                    let my_key = idl.authority;
                    let target_key = authority.key();
                    if my_key != target_key {
                        return Err(
                            anchor_lang::error::Error::from(
                                    anchor_lang::error::ErrorCode::ConstraintHasOne,
                                )
                                .with_account_name("idl")
                                .with_pubkeys((my_key, target_key)),
                        );
                    }
                }
                if !(authority.key != &ERASED_AUTHORITY) {
                    return Err(
                        anchor_lang::error::Error::from(
                                anchor_lang::error::ErrorCode::ConstraintRaw,
                            )
                            .with_account_name("authority"),
                    );
                }
                Ok(IdlSetBuffer {
                    buffer,
                    idl,
                    authority,
                })
            }
        }
        #[automatically_derived]
        impl<'info> anchor_lang::ToAccountInfos<'info> for IdlSetBuffer<'info>
        where
            'info: 'info,
        {
            fn to_account_infos(
                &self,
            ) -> Vec<anchor_lang::solana_program::account_info::AccountInfo<'info>> {
                let mut account_infos = ::alloc::vec::Vec::new();
                account_infos.extend(self.buffer.to_account_infos());
                account_infos.extend(self.idl.to_account_infos());
                account_infos.extend(self.authority.to_account_infos());
                account_infos
            }
        }
        #[automatically_derived]
        impl<'info> anchor_lang::ToAccountMetas for IdlSetBuffer<'info> {
            fn to_account_metas(
                &self,
                is_signer: Option<bool>,
            ) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
                let mut account_metas = ::alloc::vec::Vec::new();
                account_metas.extend(self.buffer.to_account_metas(None));
                account_metas.extend(self.idl.to_account_metas(None));
                account_metas.extend(self.authority.to_account_metas(None));
                account_metas
            }
        }
        #[automatically_derived]
        impl<'info> anchor_lang::AccountsExit<'info> for IdlSetBuffer<'info>
        where
            'info: 'info,
        {
            fn exit(
                &self,
                program_id: &anchor_lang::solana_program::pubkey::Pubkey,
            ) -> anchor_lang::Result<()> {
                anchor_lang::AccountsExit::exit(&self.buffer, program_id)
                    .map_err(|e| e.with_account_name("buffer"))?;
                anchor_lang::AccountsExit::exit(&self.idl, program_id)
                    .map_err(|e| e.with_account_name("idl"))?;
                Ok(())
            }
        }
        pub struct IdlSetBufferBumps {}
        #[automatically_derived]
        impl ::core::fmt::Debug for IdlSetBufferBumps {
            #[inline]
            fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                ::core::fmt::Formatter::write_str(f, "IdlSetBufferBumps")
            }
        }
        impl Default for IdlSetBufferBumps {
            fn default() -> Self {
                IdlSetBufferBumps {}
            }
        }
        impl<'info> anchor_lang::Bumps for IdlSetBuffer<'info>
        where
            'info: 'info,
        {
            type Bumps = IdlSetBufferBumps;
        }
        /// An internal, Anchor generated module. This is used (as an
        /// implementation detail), to generate a struct for a given
        /// `#[derive(Accounts)]` implementation, where each field is a Pubkey,
        /// instead of an `AccountInfo`. This is useful for clients that want
        /// to generate a list of accounts, without explicitly knowing the
        /// order all the fields should be in.
        ///
        /// To access the struct in this module, one should use the sibling
        /// `accounts` module (also generated), which re-exports this.
        pub(crate) mod __client_accounts_idl_set_buffer {
            use super::*;
            use anchor_lang::prelude::borsh;
            /// Generated client accounts for [`IdlSetBuffer`].
            pub struct IdlSetBuffer {
                pub buffer: Pubkey,
                pub idl: Pubkey,
                pub authority: Pubkey,
            }
            impl borsh::ser::BorshSerialize for IdlSetBuffer
            where
                Pubkey: borsh::ser::BorshSerialize,
                Pubkey: borsh::ser::BorshSerialize,
                Pubkey: borsh::ser::BorshSerialize,
            {
                fn serialize<W: borsh::maybestd::io::Write>(
                    &self,
                    writer: &mut W,
                ) -> ::core::result::Result<(), borsh::maybestd::io::Error> {
                    borsh::BorshSerialize::serialize(&self.buffer, writer)?;
                    borsh::BorshSerialize::serialize(&self.idl, writer)?;
                    borsh::BorshSerialize::serialize(&self.authority, writer)?;
                    Ok(())
                }
            }
            #[automatically_derived]
            impl anchor_lang::ToAccountMetas for IdlSetBuffer {
                fn to_account_metas(
                    &self,
                    is_signer: Option<bool>,
                ) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
                    let mut account_metas = ::alloc::vec::Vec::new();
                    account_metas
                        .push(
                            anchor_lang::solana_program::instruction::AccountMeta::new(
                                self.buffer,
                                false,
                            ),
                        );
                    account_metas
                        .push(
                            anchor_lang::solana_program::instruction::AccountMeta::new(
                                self.idl,
                                false,
                            ),
                        );
                    account_metas
                        .push(
                            anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                                self.authority,
                                true,
                            ),
                        );
                    account_metas
                }
            }
        }
        /// An internal, Anchor generated module. This is used (as an
        /// implementation detail), to generate a CPI struct for a given
        /// `#[derive(Accounts)]` implementation, where each field is an
        /// AccountInfo.
        ///
        /// To access the struct in this module, one should use the sibling
        /// [`cpi::accounts`] module (also generated), which re-exports this.
        pub(crate) mod __cpi_client_accounts_idl_set_buffer {
            use super::*;
            /// Generated CPI struct of the accounts for [`IdlSetBuffer`].
            pub struct IdlSetBuffer<'info> {
                pub buffer: anchor_lang::solana_program::account_info::AccountInfo<
                    'info,
                >,
                pub idl: anchor_lang::solana_program::account_info::AccountInfo<'info>,
                pub authority: anchor_lang::solana_program::account_info::AccountInfo<
                    'info,
                >,
            }
            #[automatically_derived]
            impl<'info> anchor_lang::ToAccountMetas for IdlSetBuffer<'info> {
                fn to_account_metas(
                    &self,
                    is_signer: Option<bool>,
                ) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
                    let mut account_metas = ::alloc::vec::Vec::new();
                    account_metas
                        .push(
                            anchor_lang::solana_program::instruction::AccountMeta::new(
                                anchor_lang::Key::key(&self.buffer),
                                false,
                            ),
                        );
                    account_metas
                        .push(
                            anchor_lang::solana_program::instruction::AccountMeta::new(
                                anchor_lang::Key::key(&self.idl),
                                false,
                            ),
                        );
                    account_metas
                        .push(
                            anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                                anchor_lang::Key::key(&self.authority),
                                true,
                            ),
                        );
                    account_metas
                }
            }
            #[automatically_derived]
            impl<'info> anchor_lang::ToAccountInfos<'info> for IdlSetBuffer<'info> {
                fn to_account_infos(
                    &self,
                ) -> Vec<anchor_lang::solana_program::account_info::AccountInfo<'info>> {
                    let mut account_infos = ::alloc::vec::Vec::new();
                    account_infos
                        .extend(
                            anchor_lang::ToAccountInfos::to_account_infos(&self.buffer),
                        );
                    account_infos
                        .extend(
                            anchor_lang::ToAccountInfos::to_account_infos(&self.idl),
                        );
                    account_infos
                        .extend(
                            anchor_lang::ToAccountInfos::to_account_infos(
                                &self.authority,
                            ),
                        );
                    account_infos
                }
            }
        }
        pub struct IdlCloseAccount<'info> {
            #[account(mut, has_one = authority, close = sol_destination)]
            pub account: Account<'info, IdlAccount>,
            #[account(constraint = authority.key!= &ERASED_AUTHORITY)]
            pub authority: Signer<'info>,
            #[account(mut)]
            pub sol_destination: AccountInfo<'info>,
        }
        #[automatically_derived]
        impl<'info> anchor_lang::Accounts<'info, IdlCloseAccountBumps>
        for IdlCloseAccount<'info>
        where
            'info: 'info,
        {
            #[inline(never)]
            fn try_accounts(
                __program_id: &anchor_lang::solana_program::pubkey::Pubkey,
                __accounts: &mut &'info [anchor_lang::solana_program::account_info::AccountInfo<
                    'info,
                >],
                __ix_data: &[u8],
                __bumps: &mut IdlCloseAccountBumps,
                __reallocs: &mut std::collections::BTreeSet<
                    anchor_lang::solana_program::pubkey::Pubkey,
                >,
            ) -> anchor_lang::Result<Self> {
                let account: anchor_lang::accounts::account::Account<IdlAccount> = anchor_lang::Accounts::try_accounts(
                        __program_id,
                        __accounts,
                        __ix_data,
                        __bumps,
                        __reallocs,
                    )
                    .map_err(|e| e.with_account_name("account"))?;
                let authority: Signer = anchor_lang::Accounts::try_accounts(
                        __program_id,
                        __accounts,
                        __ix_data,
                        __bumps,
                        __reallocs,
                    )
                    .map_err(|e| e.with_account_name("authority"))?;
                let sol_destination: AccountInfo = anchor_lang::Accounts::try_accounts(
                        __program_id,
                        __accounts,
                        __ix_data,
                        __bumps,
                        __reallocs,
                    )
                    .map_err(|e| e.with_account_name("sol_destination"))?;
                if !AsRef::<AccountInfo>::as_ref(&account).is_writable {
                    return Err(
                        anchor_lang::error::Error::from(
                                anchor_lang::error::ErrorCode::ConstraintMut,
                            )
                            .with_account_name("account"),
                    );
                }
                {
                    let my_key = account.authority;
                    let target_key = authority.key();
                    if my_key != target_key {
                        return Err(
                            anchor_lang::error::Error::from(
                                    anchor_lang::error::ErrorCode::ConstraintHasOne,
                                )
                                .with_account_name("account")
                                .with_pubkeys((my_key, target_key)),
                        );
                    }
                }
                {
                    if account.key() == sol_destination.key() {
                        return Err(
                            anchor_lang::error::Error::from(
                                    anchor_lang::error::ErrorCode::ConstraintClose,
                                )
                                .with_account_name("account"),
                        );
                    }
                }
                if !(authority.key != &ERASED_AUTHORITY) {
                    return Err(
                        anchor_lang::error::Error::from(
                                anchor_lang::error::ErrorCode::ConstraintRaw,
                            )
                            .with_account_name("authority"),
                    );
                }
                if !&sol_destination.is_writable {
                    return Err(
                        anchor_lang::error::Error::from(
                                anchor_lang::error::ErrorCode::ConstraintMut,
                            )
                            .with_account_name("sol_destination"),
                    );
                }
                Ok(IdlCloseAccount {
                    account,
                    authority,
                    sol_destination,
                })
            }
        }
        #[automatically_derived]
        impl<'info> anchor_lang::ToAccountInfos<'info> for IdlCloseAccount<'info>
        where
            'info: 'info,
        {
            fn to_account_infos(
                &self,
            ) -> Vec<anchor_lang::solana_program::account_info::AccountInfo<'info>> {
                let mut account_infos = ::alloc::vec::Vec::new();
                account_infos.extend(self.account.to_account_infos());
                account_infos.extend(self.authority.to_account_infos());
                account_infos.extend(self.sol_destination.to_account_infos());
                account_infos
            }
        }
        #[automatically_derived]
        impl<'info> anchor_lang::ToAccountMetas for IdlCloseAccount<'info> {
            fn to_account_metas(
                &self,
                is_signer: Option<bool>,
            ) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
                let mut account_metas = ::alloc::vec::Vec::new();
                account_metas.extend(self.account.to_account_metas(None));
                account_metas.extend(self.authority.to_account_metas(None));
                account_metas.extend(self.sol_destination.to_account_metas(None));
                account_metas
            }
        }
        #[automatically_derived]
        impl<'info> anchor_lang::AccountsExit<'info> for IdlCloseAccount<'info>
        where
            'info: 'info,
        {
            fn exit(
                &self,
                program_id: &anchor_lang::solana_program::pubkey::Pubkey,
            ) -> anchor_lang::Result<()> {
                {
                    let sol_destination = &self.sol_destination;
                    anchor_lang::AccountsClose::close(
                            &self.account,
                            sol_destination.to_account_info(),
                        )
                        .map_err(|e| e.with_account_name("account"))?;
                }
                anchor_lang::AccountsExit::exit(&self.sol_destination, program_id)
                    .map_err(|e| e.with_account_name("sol_destination"))?;
                Ok(())
            }
        }
        pub struct IdlCloseAccountBumps {}
        #[automatically_derived]
        impl ::core::fmt::Debug for IdlCloseAccountBumps {
            #[inline]
            fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                ::core::fmt::Formatter::write_str(f, "IdlCloseAccountBumps")
            }
        }
        impl Default for IdlCloseAccountBumps {
            fn default() -> Self {
                IdlCloseAccountBumps {}
            }
        }
        impl<'info> anchor_lang::Bumps for IdlCloseAccount<'info>
        where
            'info: 'info,
        {
            type Bumps = IdlCloseAccountBumps;
        }
        /// An internal, Anchor generated module. This is used (as an
        /// implementation detail), to generate a struct for a given
        /// `#[derive(Accounts)]` implementation, where each field is a Pubkey,
        /// instead of an `AccountInfo`. This is useful for clients that want
        /// to generate a list of accounts, without explicitly knowing the
        /// order all the fields should be in.
        ///
        /// To access the struct in this module, one should use the sibling
        /// `accounts` module (also generated), which re-exports this.
        pub(crate) mod __client_accounts_idl_close_account {
            use super::*;
            use anchor_lang::prelude::borsh;
            /// Generated client accounts for [`IdlCloseAccount`].
            pub struct IdlCloseAccount {
                pub account: Pubkey,
                pub authority: Pubkey,
                pub sol_destination: Pubkey,
            }
            impl borsh::ser::BorshSerialize for IdlCloseAccount
            where
                Pubkey: borsh::ser::BorshSerialize,
                Pubkey: borsh::ser::BorshSerialize,
                Pubkey: borsh::ser::BorshSerialize,
            {
                fn serialize<W: borsh::maybestd::io::Write>(
                    &self,
                    writer: &mut W,
                ) -> ::core::result::Result<(), borsh::maybestd::io::Error> {
                    borsh::BorshSerialize::serialize(&self.account, writer)?;
                    borsh::BorshSerialize::serialize(&self.authority, writer)?;
                    borsh::BorshSerialize::serialize(&self.sol_destination, writer)?;
                    Ok(())
                }
            }
            #[automatically_derived]
            impl anchor_lang::ToAccountMetas for IdlCloseAccount {
                fn to_account_metas(
                    &self,
                    is_signer: Option<bool>,
                ) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
                    let mut account_metas = ::alloc::vec::Vec::new();
                    account_metas
                        .push(
                            anchor_lang::solana_program::instruction::AccountMeta::new(
                                self.account,
                                false,
                            ),
                        );
                    account_metas
                        .push(
                            anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                                self.authority,
                                true,
                            ),
                        );
                    account_metas
                        .push(
                            anchor_lang::solana_program::instruction::AccountMeta::new(
                                self.sol_destination,
                                false,
                            ),
                        );
                    account_metas
                }
            }
        }
        /// An internal, Anchor generated module. This is used (as an
        /// implementation detail), to generate a CPI struct for a given
        /// `#[derive(Accounts)]` implementation, where each field is an
        /// AccountInfo.
        ///
        /// To access the struct in this module, one should use the sibling
        /// [`cpi::accounts`] module (also generated), which re-exports this.
        pub(crate) mod __cpi_client_accounts_idl_close_account {
            use super::*;
            /// Generated CPI struct of the accounts for [`IdlCloseAccount`].
            pub struct IdlCloseAccount<'info> {
                pub account: anchor_lang::solana_program::account_info::AccountInfo<
                    'info,
                >,
                pub authority: anchor_lang::solana_program::account_info::AccountInfo<
                    'info,
                >,
                pub sol_destination: anchor_lang::solana_program::account_info::AccountInfo<
                    'info,
                >,
            }
            #[automatically_derived]
            impl<'info> anchor_lang::ToAccountMetas for IdlCloseAccount<'info> {
                fn to_account_metas(
                    &self,
                    is_signer: Option<bool>,
                ) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
                    let mut account_metas = ::alloc::vec::Vec::new();
                    account_metas
                        .push(
                            anchor_lang::solana_program::instruction::AccountMeta::new(
                                anchor_lang::Key::key(&self.account),
                                false,
                            ),
                        );
                    account_metas
                        .push(
                            anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                                anchor_lang::Key::key(&self.authority),
                                true,
                            ),
                        );
                    account_metas
                        .push(
                            anchor_lang::solana_program::instruction::AccountMeta::new(
                                anchor_lang::Key::key(&self.sol_destination),
                                false,
                            ),
                        );
                    account_metas
                }
            }
            #[automatically_derived]
            impl<'info> anchor_lang::ToAccountInfos<'info> for IdlCloseAccount<'info> {
                fn to_account_infos(
                    &self,
                ) -> Vec<anchor_lang::solana_program::account_info::AccountInfo<'info>> {
                    let mut account_infos = ::alloc::vec::Vec::new();
                    account_infos
                        .extend(
                            anchor_lang::ToAccountInfos::to_account_infos(&self.account),
                        );
                    account_infos
                        .extend(
                            anchor_lang::ToAccountInfos::to_account_infos(
                                &self.authority,
                            ),
                        );
                    account_infos
                        .extend(
                            anchor_lang::ToAccountInfos::to_account_infos(
                                &self.sol_destination,
                            ),
                        );
                    account_infos
                }
            }
        }
        use std::cell::{Ref, RefMut};
        pub trait IdlTrailingData<'info> {
            fn trailing_data(self) -> Ref<'info, [u8]>;
            fn trailing_data_mut(self) -> RefMut<'info, [u8]>;
        }
        impl<'a, 'info: 'a> IdlTrailingData<'a> for &'a Account<'info, IdlAccount> {
            fn trailing_data(self) -> Ref<'a, [u8]> {
                let info: &AccountInfo<'info> = self.as_ref();
                Ref::map(info.try_borrow_data().unwrap(), |d| &d[44..])
            }
            fn trailing_data_mut(self) -> RefMut<'a, [u8]> {
                let info: &AccountInfo<'info> = self.as_ref();
                RefMut::map(info.try_borrow_mut_data().unwrap(), |d| &mut d[44..])
            }
        }
        #[inline(never)]
        pub fn __idl_create_account(
            program_id: &Pubkey,
            accounts: &mut IdlCreateAccounts,
            data_len: u64,
        ) -> anchor_lang::Result<()> {
            ::solana_msg::sol_log("Instruction: IdlCreateAccount");
            if program_id != accounts.program.key {
                return Err(
                    anchor_lang::error::ErrorCode::IdlInstructionInvalidProgram.into(),
                );
            }
            let from = accounts.from.key;
            let (base, nonce) = Pubkey::find_program_address(&[], program_id);
            let seed = IdlAccount::seed();
            let owner = accounts.program.key;
            let to = Pubkey::create_with_seed(&base, seed, owner).unwrap();
            let space = std::cmp::min(
                IdlAccount::DISCRIMINATOR.len() + 32 + 4 + data_len as usize,
                10_000,
            );
            let rent = Rent::get()?;
            let lamports = rent.minimum_balance(space);
            let seeds = &[&[nonce][..]];
            let ix = anchor_lang::solana_program::system_instruction::create_account_with_seed(
                from,
                &to,
                &base,
                seed,
                lamports,
                space as u64,
                owner,
            );
            anchor_lang::solana_program::program::invoke_signed(
                &ix,
                &[
                    accounts.from.clone(),
                    accounts.to.clone(),
                    accounts.base.clone(),
                    accounts.system_program.to_account_info(),
                ],
                &[seeds],
            )?;
            let mut idl_account = {
                let mut account_data = accounts.to.try_borrow_data()?;
                let mut account_data_slice: &[u8] = &account_data;
                IdlAccount::try_deserialize_unchecked(&mut account_data_slice)?
            };
            idl_account.authority = *accounts.from.key;
            let mut data = accounts.to.try_borrow_mut_data()?;
            let dst: &mut [u8] = &mut data;
            let mut cursor = std::io::Cursor::new(dst);
            idl_account.try_serialize(&mut cursor)?;
            Ok(())
        }
        #[inline(never)]
        pub fn __idl_resize_account(
            program_id: &Pubkey,
            accounts: &mut IdlResizeAccount,
            data_len: u64,
        ) -> anchor_lang::Result<()> {
            ::solana_msg::sol_log("Instruction: IdlResizeAccount");
            let data_len: usize = data_len as usize;
            if accounts.idl.data_len != 0 {
                return Err(anchor_lang::error::ErrorCode::IdlAccountNotEmpty.into());
            }
            let idl_ref = AsRef::<AccountInfo>::as_ref(&accounts.idl);
            let new_account_space = idl_ref
                .data_len()
                .checked_add(
                    std::cmp::min(
                        data_len
                            .checked_sub(idl_ref.data_len())
                            .expect(
                                "data_len should always be >= the current account space",
                            ),
                        10_000,
                    ),
                )
                .unwrap();
            if new_account_space > idl_ref.data_len() {
                let sysvar_rent = Rent::get()?;
                let new_rent_minimum = sysvar_rent.minimum_balance(new_account_space);
                anchor_lang::system_program::transfer(
                    anchor_lang::context::CpiContext::new(
                        accounts.system_program.to_account_info(),
                        anchor_lang::system_program::Transfer {
                            from: accounts.authority.to_account_info(),
                            to: accounts.idl.to_account_info(),
                        },
                    ),
                    new_rent_minimum.checked_sub(idl_ref.lamports()).unwrap(),
                )?;
                idl_ref.realloc(new_account_space, false)?;
            }
            Ok(())
        }
        #[inline(never)]
        pub fn __idl_close_account(
            program_id: &Pubkey,
            accounts: &mut IdlCloseAccount,
        ) -> anchor_lang::Result<()> {
            ::solana_msg::sol_log("Instruction: IdlCloseAccount");
            Ok(())
        }
        #[inline(never)]
        pub fn __idl_create_buffer(
            program_id: &Pubkey,
            accounts: &mut IdlCreateBuffer,
        ) -> anchor_lang::Result<()> {
            ::solana_msg::sol_log("Instruction: IdlCreateBuffer");
            let mut buffer = &mut accounts.buffer;
            buffer.authority = *accounts.authority.key;
            Ok(())
        }
        #[inline(never)]
        pub fn __idl_write(
            program_id: &Pubkey,
            accounts: &mut IdlAccounts,
            idl_data: Vec<u8>,
        ) -> anchor_lang::Result<()> {
            ::solana_msg::sol_log("Instruction: IdlWrite");
            let prev_len: usize = ::std::convert::TryInto::<
                usize,
            >::try_into(accounts.idl.data_len)
                .unwrap();
            let new_len: usize = prev_len.checked_add(idl_data.len()).unwrap() as usize;
            accounts
                .idl
                .data_len = accounts
                .idl
                .data_len
                .checked_add(
                    ::std::convert::TryInto::<u32>::try_into(idl_data.len()).unwrap(),
                )
                .unwrap();
            use IdlTrailingData;
            let mut idl_bytes = accounts.idl.trailing_data_mut();
            let idl_expansion = &mut idl_bytes[prev_len..new_len];
            if idl_expansion.len() != idl_data.len() {
                return Err(
                    anchor_lang::error::Error::from(anchor_lang::error::AnchorError {
                            error_name: anchor_lang::error::ErrorCode::RequireEqViolated
                                .name(),
                            error_code_number: anchor_lang::error::ErrorCode::RequireEqViolated
                                .into(),
                            error_msg: anchor_lang::error::ErrorCode::RequireEqViolated
                                .to_string(),
                            error_origin: Some(
                                anchor_lang::error::ErrorOrigin::Source(anchor_lang::error::Source {
                                    filename: "programs/express_relay/src/lib.rs",
                                    line: 33u32,
                                }),
                            ),
                            compared_values: None,
                        })
                        .with_values((idl_expansion.len(), idl_data.len())),
                );
            }
            idl_expansion.copy_from_slice(&idl_data[..]);
            Ok(())
        }
        #[inline(never)]
        pub fn __idl_set_authority(
            program_id: &Pubkey,
            accounts: &mut IdlAccounts,
            new_authority: Pubkey,
        ) -> anchor_lang::Result<()> {
            ::solana_msg::sol_log("Instruction: IdlSetAuthority");
            accounts.idl.authority = new_authority;
            Ok(())
        }
        #[inline(never)]
        pub fn __idl_set_buffer(
            program_id: &Pubkey,
            accounts: &mut IdlSetBuffer,
        ) -> anchor_lang::Result<()> {
            ::solana_msg::sol_log("Instruction: IdlSetBuffer");
            accounts.idl.data_len = accounts.buffer.data_len;
            use IdlTrailingData;
            let buffer_len = ::std::convert::TryInto::<
                usize,
            >::try_into(accounts.buffer.data_len)
                .unwrap();
            let mut target = accounts.idl.trailing_data_mut();
            let source = &accounts.buffer.trailing_data()[..buffer_len];
            if target.len() < buffer_len {
                return Err(
                    anchor_lang::error::Error::from(anchor_lang::error::AnchorError {
                            error_name: anchor_lang::error::ErrorCode::RequireGteViolated
                                .name(),
                            error_code_number: anchor_lang::error::ErrorCode::RequireGteViolated
                                .into(),
                            error_msg: anchor_lang::error::ErrorCode::RequireGteViolated
                                .to_string(),
                            error_origin: Some(
                                anchor_lang::error::ErrorOrigin::Source(anchor_lang::error::Source {
                                    filename: "programs/express_relay/src/lib.rs",
                                    line: 33u32,
                                }),
                            ),
                            compared_values: None,
                        })
                        .with_values((target.len(), buffer_len)),
                );
            }
            target[..buffer_len].copy_from_slice(source);
            Ok(())
        }
    }
    /// __global mod defines wrapped handlers for global instructions.
    pub mod __global {
        use super::*;
        #[inline(never)]
        pub fn initialize<'info>(
            __program_id: &Pubkey,
            __accounts: &'info [AccountInfo<'info>],
            __ix_data: &[u8],
        ) -> anchor_lang::Result<()> {
            ::solana_msg::sol_log("Instruction: Initialize");
            let ix = instruction::Initialize::deserialize(&mut &__ix_data[..])
                .map_err(|_| {
                    anchor_lang::error::ErrorCode::InstructionDidNotDeserialize
                })?;
            let instruction::Initialize { data } = ix;
            let mut __bumps = <Initialize as anchor_lang::Bumps>::Bumps::default();
            let mut __reallocs = std::collections::BTreeSet::new();
            let mut __remaining_accounts: &[AccountInfo] = __accounts;
            let mut __accounts = Initialize::try_accounts(
                __program_id,
                &mut __remaining_accounts,
                __ix_data,
                &mut __bumps,
                &mut __reallocs,
            )?;
            let result = express_relay::initialize(
                anchor_lang::context::Context::new(
                    __program_id,
                    &mut __accounts,
                    __remaining_accounts,
                    __bumps,
                ),
                data,
            )?;
            __accounts.exit(__program_id)
        }
        #[inline(never)]
        pub fn set_admin<'info>(
            __program_id: &Pubkey,
            __accounts: &'info [AccountInfo<'info>],
            __ix_data: &[u8],
        ) -> anchor_lang::Result<()> {
            ::solana_msg::sol_log("Instruction: SetAdmin");
            let ix = instruction::SetAdmin::deserialize(&mut &__ix_data[..])
                .map_err(|_| {
                    anchor_lang::error::ErrorCode::InstructionDidNotDeserialize
                })?;
            let instruction::SetAdmin = ix;
            let mut __bumps = <SetAdmin as anchor_lang::Bumps>::Bumps::default();
            let mut __reallocs = std::collections::BTreeSet::new();
            let mut __remaining_accounts: &[AccountInfo] = __accounts;
            let mut __accounts = SetAdmin::try_accounts(
                __program_id,
                &mut __remaining_accounts,
                __ix_data,
                &mut __bumps,
                &mut __reallocs,
            )?;
            let result = express_relay::set_admin(
                anchor_lang::context::Context::new(
                    __program_id,
                    &mut __accounts,
                    __remaining_accounts,
                    __bumps,
                ),
            )?;
            __accounts.exit(__program_id)
        }
        #[inline(never)]
        pub fn set_relayer<'info>(
            __program_id: &Pubkey,
            __accounts: &'info [AccountInfo<'info>],
            __ix_data: &[u8],
        ) -> anchor_lang::Result<()> {
            ::solana_msg::sol_log("Instruction: SetRelayer");
            let ix = instruction::SetRelayer::deserialize(&mut &__ix_data[..])
                .map_err(|_| {
                    anchor_lang::error::ErrorCode::InstructionDidNotDeserialize
                })?;
            let instruction::SetRelayer = ix;
            let mut __bumps = <SetRelayer as anchor_lang::Bumps>::Bumps::default();
            let mut __reallocs = std::collections::BTreeSet::new();
            let mut __remaining_accounts: &[AccountInfo] = __accounts;
            let mut __accounts = SetRelayer::try_accounts(
                __program_id,
                &mut __remaining_accounts,
                __ix_data,
                &mut __bumps,
                &mut __reallocs,
            )?;
            let result = express_relay::set_relayer(
                anchor_lang::context::Context::new(
                    __program_id,
                    &mut __accounts,
                    __remaining_accounts,
                    __bumps,
                ),
            )?;
            __accounts.exit(__program_id)
        }
        #[inline(never)]
        pub fn set_splits<'info>(
            __program_id: &Pubkey,
            __accounts: &'info [AccountInfo<'info>],
            __ix_data: &[u8],
        ) -> anchor_lang::Result<()> {
            ::solana_msg::sol_log("Instruction: SetSplits");
            let ix = instruction::SetSplits::deserialize(&mut &__ix_data[..])
                .map_err(|_| {
                    anchor_lang::error::ErrorCode::InstructionDidNotDeserialize
                })?;
            let instruction::SetSplits { data } = ix;
            let mut __bumps = <SetSplits as anchor_lang::Bumps>::Bumps::default();
            let mut __reallocs = std::collections::BTreeSet::new();
            let mut __remaining_accounts: &[AccountInfo] = __accounts;
            let mut __accounts = SetSplits::try_accounts(
                __program_id,
                &mut __remaining_accounts,
                __ix_data,
                &mut __bumps,
                &mut __reallocs,
            )?;
            let result = express_relay::set_splits(
                anchor_lang::context::Context::new(
                    __program_id,
                    &mut __accounts,
                    __remaining_accounts,
                    __bumps,
                ),
                data,
            )?;
            __accounts.exit(__program_id)
        }
        #[inline(never)]
        pub fn set_router_split<'info>(
            __program_id: &Pubkey,
            __accounts: &'info [AccountInfo<'info>],
            __ix_data: &[u8],
        ) -> anchor_lang::Result<()> {
            ::solana_msg::sol_log("Instruction: SetRouterSplit");
            let ix = instruction::SetRouterSplit::deserialize(&mut &__ix_data[..])
                .map_err(|_| {
                    anchor_lang::error::ErrorCode::InstructionDidNotDeserialize
                })?;
            let instruction::SetRouterSplit { data } = ix;
            let mut __bumps = <SetRouterSplit as anchor_lang::Bumps>::Bumps::default();
            let mut __reallocs = std::collections::BTreeSet::new();
            let mut __remaining_accounts: &[AccountInfo] = __accounts;
            let mut __accounts = SetRouterSplit::try_accounts(
                __program_id,
                &mut __remaining_accounts,
                __ix_data,
                &mut __bumps,
                &mut __reallocs,
            )?;
            let result = express_relay::set_router_split(
                anchor_lang::context::Context::new(
                    __program_id,
                    &mut __accounts,
                    __remaining_accounts,
                    __bumps,
                ),
                data,
            )?;
            __accounts.exit(__program_id)
        }
        #[inline(never)]
        pub fn submit_bid<'info>(
            __program_id: &Pubkey,
            __accounts: &'info [AccountInfo<'info>],
            __ix_data: &[u8],
        ) -> anchor_lang::Result<()> {
            ::solana_msg::sol_log("Instruction: SubmitBid");
            let ix = instruction::SubmitBid::deserialize(&mut &__ix_data[..])
                .map_err(|_| {
                    anchor_lang::error::ErrorCode::InstructionDidNotDeserialize
                })?;
            let instruction::SubmitBid { data } = ix;
            let mut __bumps = <SubmitBid as anchor_lang::Bumps>::Bumps::default();
            let mut __reallocs = std::collections::BTreeSet::new();
            let mut __remaining_accounts: &[AccountInfo] = __accounts;
            let mut __accounts = SubmitBid::try_accounts(
                __program_id,
                &mut __remaining_accounts,
                __ix_data,
                &mut __bumps,
                &mut __reallocs,
            )?;
            let result = express_relay::submit_bid(
                anchor_lang::context::Context::new(
                    __program_id,
                    &mut __accounts,
                    __remaining_accounts,
                    __bumps,
                ),
                data,
            )?;
            __accounts.exit(__program_id)
        }
        #[inline(never)]
        pub fn check_permission<'info>(
            __program_id: &Pubkey,
            __accounts: &'info [AccountInfo<'info>],
            __ix_data: &[u8],
        ) -> anchor_lang::Result<()> {
            ::solana_msg::sol_log("Instruction: CheckPermission");
            let ix = instruction::CheckPermission::deserialize(&mut &__ix_data[..])
                .map_err(|_| {
                    anchor_lang::error::ErrorCode::InstructionDidNotDeserialize
                })?;
            let instruction::CheckPermission = ix;
            let mut __bumps = <CheckPermission as anchor_lang::Bumps>::Bumps::default();
            let mut __reallocs = std::collections::BTreeSet::new();
            let mut __remaining_accounts: &[AccountInfo] = __accounts;
            let mut __accounts = CheckPermission::try_accounts(
                __program_id,
                &mut __remaining_accounts,
                __ix_data,
                &mut __bumps,
                &mut __reallocs,
            )?;
            let result = express_relay::check_permission(
                anchor_lang::context::Context::new(
                    __program_id,
                    &mut __accounts,
                    __remaining_accounts,
                    __bumps,
                ),
            )?;
            let mut return_data = Vec::with_capacity(256);
            result.serialize(&mut return_data).unwrap();
            anchor_lang::solana_program::program::set_return_data(&return_data);
            __accounts.exit(__program_id)
        }
        #[inline(never)]
        pub fn withdraw_fees<'info>(
            __program_id: &Pubkey,
            __accounts: &'info [AccountInfo<'info>],
            __ix_data: &[u8],
        ) -> anchor_lang::Result<()> {
            ::solana_msg::sol_log("Instruction: WithdrawFees");
            let ix = instruction::WithdrawFees::deserialize(&mut &__ix_data[..])
                .map_err(|_| {
                    anchor_lang::error::ErrorCode::InstructionDidNotDeserialize
                })?;
            let instruction::WithdrawFees = ix;
            let mut __bumps = <WithdrawFees as anchor_lang::Bumps>::Bumps::default();
            let mut __reallocs = std::collections::BTreeSet::new();
            let mut __remaining_accounts: &[AccountInfo] = __accounts;
            let mut __accounts = WithdrawFees::try_accounts(
                __program_id,
                &mut __remaining_accounts,
                __ix_data,
                &mut __bumps,
                &mut __reallocs,
            )?;
            let result = express_relay::withdraw_fees(
                anchor_lang::context::Context::new(
                    __program_id,
                    &mut __accounts,
                    __remaining_accounts,
                    __bumps,
                ),
            )?;
            __accounts.exit(__program_id)
        }
        #[inline(never)]
        pub fn swap<'info>(
            __program_id: &Pubkey,
            __accounts: &'info [AccountInfo<'info>],
            __ix_data: &[u8],
        ) -> anchor_lang::Result<()> {
            ::solana_msg::sol_log("Instruction: Swap");
            let ix = instruction::Swap::deserialize(&mut &__ix_data[..])
                .map_err(|_| {
                    anchor_lang::error::ErrorCode::InstructionDidNotDeserialize
                })?;
            let instruction::Swap { data } = ix;
            let mut __bumps = <Swap as anchor_lang::Bumps>::Bumps::default();
            let mut __reallocs = std::collections::BTreeSet::new();
            let mut __remaining_accounts: &[AccountInfo] = __accounts;
            let mut __accounts = Swap::try_accounts(
                __program_id,
                &mut __remaining_accounts,
                __ix_data,
                &mut __bumps,
                &mut __reallocs,
            )?;
            let result = express_relay::swap(
                anchor_lang::context::Context::new(
                    __program_id,
                    &mut __accounts,
                    __remaining_accounts,
                    __bumps,
                ),
                data,
            )?;
            __accounts.exit(__program_id)
        }
    }
}
pub mod express_relay {
    use {super::*, token::{check_ata, check_mint, transfer_token_if_needed}};
    pub fn initialize(ctx: Context<Initialize>, data: InitializeArgs) -> Result<()> {
        validate_fee_split(data.split_router_default)?;
        validate_fee_split(data.split_relayer)?;
        let express_relay_metadata_data = &mut ctx.accounts.express_relay_metadata;
        express_relay_metadata_data.admin = *ctx.accounts.admin.key;
        express_relay_metadata_data.relayer_signer = *ctx.accounts.relayer_signer.key;
        express_relay_metadata_data
            .fee_receiver_relayer = *ctx.accounts.fee_receiver_relayer.key;
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
        express_relay_metadata_data
            .fee_receiver_relayer = *ctx.accounts.fee_receiver_relayer.key;
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
    pub fn set_router_split(
        ctx: Context<SetRouterSplit>,
        data: SetRouterSplitArgs,
    ) -> Result<()> {
        validate_fee_split(data.split_router)?;
        ctx.accounts.config_router.router = *ctx.accounts.router.key;
        ctx.accounts.config_router.split = data.split_router;
        Ok(())
    }
    /// Submits a bid for a particular (permission, router) pair and distributes bids according to splits.
    pub fn submit_bid(ctx: Context<SubmitBid>, data: SubmitBidArgs) -> Result<()> {
        if data.deadline < Clock::get()?.unix_timestamp {
            return Err(
                anchor_lang::error::Error::from(anchor_lang::error::AnchorError {
                    error_name: ErrorCode::DeadlinePassed.name(),
                    error_code_number: ErrorCode::DeadlinePassed.into(),
                    error_msg: ErrorCode::DeadlinePassed.to_string(),
                    error_origin: Some(
                        anchor_lang::error::ErrorOrigin::Source(anchor_lang::error::Source {
                            filename: "programs/express_relay/src/lib.rs",
                            line: 100u32,
                        }),
                    ),
                    compared_values: None,
                }),
            );
        }
        if get_stack_height() > TRANSACTION_LEVEL_STACK_HEIGHT {
            return Err(
                anchor_lang::error::Error::from(anchor_lang::error::AnchorError {
                    error_name: ErrorCode::InvalidCPISubmitBid.name(),
                    error_code_number: ErrorCode::InvalidCPISubmitBid.into(),
                    error_msg: ErrorCode::InvalidCPISubmitBid.to_string(),
                    error_origin: Some(
                        anchor_lang::error::ErrorOrigin::Source(anchor_lang::error::Source {
                            filename: "programs/express_relay/src/lib.rs",
                            line: 105u32,
                        }),
                    ),
                    compared_values: None,
                }),
            );
        }
        let matching_ixs = get_matching_submit_bid_instructions(
            ctx.accounts.sysvar_instructions.to_account_info(),
            None,
        )?;
        if matching_ixs.len() > 1 {
            return Err(
                anchor_lang::error::Error::from(anchor_lang::error::AnchorError {
                    error_name: ErrorCode::MultiplePermissions.name(),
                    error_code_number: ErrorCode::MultiplePermissions.into(),
                    error_msg: ErrorCode::MultiplePermissions.to_string(),
                    error_origin: Some(
                        anchor_lang::error::ErrorOrigin::Source(anchor_lang::error::Source {
                            filename: "programs/express_relay/src/lib.rs",
                            line: 116u32,
                        }),
                    ),
                    compared_values: None,
                }),
            );
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
                permission: *ctx.accounts.permission.key,
                router: *ctx.accounts.router.key,
                config_router: ctx.accounts.config_router.to_account_info(),
                express_relay_metadata: ctx
                    .accounts
                    .express_relay_metadata
                    .to_account_info(),
            },
        )?;
        if num_permissions == 0 {
            return Err(
                anchor_lang::error::Error::from(anchor_lang::error::AnchorError {
                    error_name: ErrorCode::MissingPermission.name(),
                    error_code_number: ErrorCode::MissingPermission.into(),
                    error_msg: ErrorCode::MissingPermission.to_string(),
                    error_origin: Some(
                        anchor_lang::error::ErrorOrigin::Source(anchor_lang::error::Source {
                            filename: "programs/express_relay/src/lib.rs",
                            line: 137u32,
                        }),
                    ),
                    compared_values: None,
                }),
            );
        }
        Ok(total_router_fees)
    }
    pub fn withdraw_fees(ctx: Context<WithdrawFees>) -> Result<()> {
        let express_relay_metadata = &ctx.accounts.express_relay_metadata;
        let fee_receiver_admin = &ctx.accounts.fee_receiver_admin;
        let express_relay_metadata_account_info = express_relay_metadata
            .to_account_info();
        let rent_express_relay_metadata = Rent::get()?
            .minimum_balance(express_relay_metadata_account_info.data_len());
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
        let (
            input_after_fees,
            output_after_fees,
            send_fee_args,
        ): (u64, u64, SendFeeArgs) = match data.fee_token {
            FeeToken::Input => {
                (
                    data.amount_input,
                    data.amount_output,
                    SendFeeArgs {
                        fee_express_relay: 0,
                        fee_relayer: 0,
                        fee_router: 0,
                        mint: ctx.accounts.mint_input.clone(),
                        token_program: ctx.accounts.token_program_input.clone(),
                        from: ctx.accounts.searcher_input_ta.clone(),
                        authority: ctx.accounts.searcher.clone(),
                    },
                )
            }
            FeeToken::Output => {
                (
                    data.amount_input,
                    data.amount_output,
                    SendFeeArgs {
                        fee_express_relay: 0,
                        fee_relayer: 0,
                        fee_router: 0,
                        from: ctx.accounts.trader_output_ata.clone(),
                        authority: ctx.accounts.trader.clone(),
                        mint: ctx.accounts.mint_output.clone(),
                        token_program: ctx.accounts.token_program_output.clone(),
                    },
                )
            }
        };
        check_ata(
            &ctx.accounts.express_relay_fee_receiver_ata,
            &ctx.accounts.express_relay_metadata.key(),
            &send_fee_args.mint,
        )?;
        check_ata(
            &ctx.accounts.relayer_fee_receiver_ata,
            &ctx.accounts.express_relay_metadata.relayer_signer,
            &send_fee_args.mint,
        )?;
        check_mint(&ctx.accounts.router_fee_receiver_ata, &send_fee_args.mint)?;
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
            &ctx.accounts.router_fee_receiver_ata,
            &send_fee_args.token_program,
            &send_fee_args.authority,
            &send_fee_args.mint,
            send_fee_args.fee_router,
        )?;
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
/// An Anchor generated module containing the program's set of
/// instructions, where each method handler in the `#[program]` mod is
/// associated with a struct defining the input arguments to the
/// method. These should be used directly, when one wants to serialize
/// Anchor instruction data, for example, when speciying
/// instructions on a client.
pub mod instruction {
    use super::*;
    /// Instruction.
    pub struct Initialize {
        pub data: InitializeArgs,
    }
    impl borsh::ser::BorshSerialize for Initialize
    where
        InitializeArgs: borsh::ser::BorshSerialize,
    {
        fn serialize<W: borsh::maybestd::io::Write>(
            &self,
            writer: &mut W,
        ) -> ::core::result::Result<(), borsh::maybestd::io::Error> {
            borsh::BorshSerialize::serialize(&self.data, writer)?;
            Ok(())
        }
    }
    impl borsh::de::BorshDeserialize for Initialize
    where
        InitializeArgs: borsh::BorshDeserialize,
    {
        fn deserialize_reader<R: borsh::maybestd::io::Read>(
            reader: &mut R,
        ) -> ::core::result::Result<Self, borsh::maybestd::io::Error> {
            Ok(Self {
                data: borsh::BorshDeserialize::deserialize_reader(reader)?,
            })
        }
    }
    impl anchor_lang::Discriminator for Initialize {
        const DISCRIMINATOR: &'static [u8] = &[175, 175, 109, 31, 13, 152, 155, 237];
    }
    impl anchor_lang::InstructionData for Initialize {}
    impl anchor_lang::Owner for Initialize {
        fn owner() -> Pubkey {
            ID
        }
    }
    /// Instruction.
    pub struct SetAdmin;
    impl borsh::ser::BorshSerialize for SetAdmin {
        fn serialize<W: borsh::maybestd::io::Write>(
            &self,
            writer: &mut W,
        ) -> ::core::result::Result<(), borsh::maybestd::io::Error> {
            Ok(())
        }
    }
    impl borsh::de::BorshDeserialize for SetAdmin {
        fn deserialize_reader<R: borsh::maybestd::io::Read>(
            reader: &mut R,
        ) -> ::core::result::Result<Self, borsh::maybestd::io::Error> {
            Ok(Self {})
        }
    }
    impl anchor_lang::Discriminator for SetAdmin {
        const DISCRIMINATOR: &'static [u8] = &[251, 163, 0, 52, 91, 194, 187, 92];
    }
    impl anchor_lang::InstructionData for SetAdmin {}
    impl anchor_lang::Owner for SetAdmin {
        fn owner() -> Pubkey {
            ID
        }
    }
    /// Instruction.
    pub struct SetRelayer;
    impl borsh::ser::BorshSerialize for SetRelayer {
        fn serialize<W: borsh::maybestd::io::Write>(
            &self,
            writer: &mut W,
        ) -> ::core::result::Result<(), borsh::maybestd::io::Error> {
            Ok(())
        }
    }
    impl borsh::de::BorshDeserialize for SetRelayer {
        fn deserialize_reader<R: borsh::maybestd::io::Read>(
            reader: &mut R,
        ) -> ::core::result::Result<Self, borsh::maybestd::io::Error> {
            Ok(Self {})
        }
    }
    impl anchor_lang::Discriminator for SetRelayer {
        const DISCRIMINATOR: &'static [u8] = &[23, 243, 33, 88, 110, 84, 196, 37];
    }
    impl anchor_lang::InstructionData for SetRelayer {}
    impl anchor_lang::Owner for SetRelayer {
        fn owner() -> Pubkey {
            ID
        }
    }
    /// Instruction.
    pub struct SetSplits {
        pub data: SetSplitsArgs,
    }
    impl borsh::ser::BorshSerialize for SetSplits
    where
        SetSplitsArgs: borsh::ser::BorshSerialize,
    {
        fn serialize<W: borsh::maybestd::io::Write>(
            &self,
            writer: &mut W,
        ) -> ::core::result::Result<(), borsh::maybestd::io::Error> {
            borsh::BorshSerialize::serialize(&self.data, writer)?;
            Ok(())
        }
    }
    impl borsh::de::BorshDeserialize for SetSplits
    where
        SetSplitsArgs: borsh::BorshDeserialize,
    {
        fn deserialize_reader<R: borsh::maybestd::io::Read>(
            reader: &mut R,
        ) -> ::core::result::Result<Self, borsh::maybestd::io::Error> {
            Ok(Self {
                data: borsh::BorshDeserialize::deserialize_reader(reader)?,
            })
        }
    }
    impl anchor_lang::Discriminator for SetSplits {
        const DISCRIMINATOR: &'static [u8] = &[175, 2, 86, 49, 225, 202, 232, 189];
    }
    impl anchor_lang::InstructionData for SetSplits {}
    impl anchor_lang::Owner for SetSplits {
        fn owner() -> Pubkey {
            ID
        }
    }
    /// Instruction.
    pub struct SetRouterSplit {
        pub data: SetRouterSplitArgs,
    }
    impl borsh::ser::BorshSerialize for SetRouterSplit
    where
        SetRouterSplitArgs: borsh::ser::BorshSerialize,
    {
        fn serialize<W: borsh::maybestd::io::Write>(
            &self,
            writer: &mut W,
        ) -> ::core::result::Result<(), borsh::maybestd::io::Error> {
            borsh::BorshSerialize::serialize(&self.data, writer)?;
            Ok(())
        }
    }
    impl borsh::de::BorshDeserialize for SetRouterSplit
    where
        SetRouterSplitArgs: borsh::BorshDeserialize,
    {
        fn deserialize_reader<R: borsh::maybestd::io::Read>(
            reader: &mut R,
        ) -> ::core::result::Result<Self, borsh::maybestd::io::Error> {
            Ok(Self {
                data: borsh::BorshDeserialize::deserialize_reader(reader)?,
            })
        }
    }
    impl anchor_lang::Discriminator for SetRouterSplit {
        const DISCRIMINATOR: &'static [u8] = &[16, 150, 106, 13, 27, 191, 104, 8];
    }
    impl anchor_lang::InstructionData for SetRouterSplit {}
    impl anchor_lang::Owner for SetRouterSplit {
        fn owner() -> Pubkey {
            ID
        }
    }
    /// Instruction.
    pub struct SubmitBid {
        pub data: SubmitBidArgs,
    }
    impl borsh::ser::BorshSerialize for SubmitBid
    where
        SubmitBidArgs: borsh::ser::BorshSerialize,
    {
        fn serialize<W: borsh::maybestd::io::Write>(
            &self,
            writer: &mut W,
        ) -> ::core::result::Result<(), borsh::maybestd::io::Error> {
            borsh::BorshSerialize::serialize(&self.data, writer)?;
            Ok(())
        }
    }
    impl borsh::de::BorshDeserialize for SubmitBid
    where
        SubmitBidArgs: borsh::BorshDeserialize,
    {
        fn deserialize_reader<R: borsh::maybestd::io::Read>(
            reader: &mut R,
        ) -> ::core::result::Result<Self, borsh::maybestd::io::Error> {
            Ok(Self {
                data: borsh::BorshDeserialize::deserialize_reader(reader)?,
            })
        }
    }
    impl anchor_lang::Discriminator for SubmitBid {
        const DISCRIMINATOR: &'static [u8] = &[19, 164, 237, 254, 64, 139, 237, 93];
    }
    impl anchor_lang::InstructionData for SubmitBid {}
    impl anchor_lang::Owner for SubmitBid {
        fn owner() -> Pubkey {
            ID
        }
    }
    /// Instruction.
    pub struct CheckPermission;
    impl borsh::ser::BorshSerialize for CheckPermission {
        fn serialize<W: borsh::maybestd::io::Write>(
            &self,
            writer: &mut W,
        ) -> ::core::result::Result<(), borsh::maybestd::io::Error> {
            Ok(())
        }
    }
    impl borsh::de::BorshDeserialize for CheckPermission {
        fn deserialize_reader<R: borsh::maybestd::io::Read>(
            reader: &mut R,
        ) -> ::core::result::Result<Self, borsh::maybestd::io::Error> {
            Ok(Self {})
        }
    }
    impl anchor_lang::Discriminator for CheckPermission {
        const DISCRIMINATOR: &'static [u8] = &[154, 199, 232, 242, 96, 72, 197, 236];
    }
    impl anchor_lang::InstructionData for CheckPermission {}
    impl anchor_lang::Owner for CheckPermission {
        fn owner() -> Pubkey {
            ID
        }
    }
    /// Instruction.
    pub struct WithdrawFees;
    impl borsh::ser::BorshSerialize for WithdrawFees {
        fn serialize<W: borsh::maybestd::io::Write>(
            &self,
            writer: &mut W,
        ) -> ::core::result::Result<(), borsh::maybestd::io::Error> {
            Ok(())
        }
    }
    impl borsh::de::BorshDeserialize for WithdrawFees {
        fn deserialize_reader<R: borsh::maybestd::io::Read>(
            reader: &mut R,
        ) -> ::core::result::Result<Self, borsh::maybestd::io::Error> {
            Ok(Self {})
        }
    }
    impl anchor_lang::Discriminator for WithdrawFees {
        const DISCRIMINATOR: &'static [u8] = &[198, 212, 171, 109, 144, 215, 174, 89];
    }
    impl anchor_lang::InstructionData for WithdrawFees {}
    impl anchor_lang::Owner for WithdrawFees {
        fn owner() -> Pubkey {
            ID
        }
    }
    /// Instruction.
    pub struct Swap {
        pub data: SwapArgs,
    }
    impl borsh::ser::BorshSerialize for Swap
    where
        SwapArgs: borsh::ser::BorshSerialize,
    {
        fn serialize<W: borsh::maybestd::io::Write>(
            &self,
            writer: &mut W,
        ) -> ::core::result::Result<(), borsh::maybestd::io::Error> {
            borsh::BorshSerialize::serialize(&self.data, writer)?;
            Ok(())
        }
    }
    impl borsh::de::BorshDeserialize for Swap
    where
        SwapArgs: borsh::BorshDeserialize,
    {
        fn deserialize_reader<R: borsh::maybestd::io::Read>(
            reader: &mut R,
        ) -> ::core::result::Result<Self, borsh::maybestd::io::Error> {
            Ok(Self {
                data: borsh::BorshDeserialize::deserialize_reader(reader)?,
            })
        }
    }
    impl anchor_lang::Discriminator for Swap {
        const DISCRIMINATOR: &'static [u8] = &[248, 198, 158, 145, 225, 117, 135, 200];
    }
    impl anchor_lang::InstructionData for Swap {}
    impl anchor_lang::Owner for Swap {
        fn owner() -> Pubkey {
            ID
        }
    }
}
#[cfg(feature = "cpi")]
pub mod cpi {
    use super::*;
    use std::marker::PhantomData;
    pub struct Return<T> {
        phantom: std::marker::PhantomData<T>,
    }
    impl<T: AnchorDeserialize> Return<T> {
        pub fn get(&self) -> T {
            let (_key, data) = anchor_lang::solana_program::program::get_return_data()
                .unwrap();
            T::try_from_slice(&data).unwrap()
        }
    }
    pub fn initialize<'a, 'b, 'c, 'info>(
        ctx: anchor_lang::context::CpiContext<
            'a,
            'b,
            'c,
            'info,
            crate::cpi::accounts::Initialize<'info>,
        >,
        data: InitializeArgs,
    ) -> anchor_lang::Result<()> {
        let ix = {
            let ix = instruction::Initialize { data };
            let mut data = Vec::with_capacity(256);
            data.extend_from_slice(
                <instruction::Initialize as anchor_lang::Discriminator>::DISCRIMINATOR,
            );
            AnchorSerialize::serialize(&ix, &mut data)
                .map_err(|_| anchor_lang::error::ErrorCode::InstructionDidNotSerialize)?;
            let accounts = ctx.to_account_metas(None);
            anchor_lang::solana_program::instruction::Instruction {
                program_id: ctx.program.key(),
                accounts,
                data,
            }
        };
        let mut acc_infos = ctx.to_account_infos();
        anchor_lang::solana_program::program::invoke_signed(
                &ix,
                &acc_infos,
                ctx.signer_seeds,
            )
            .map_or_else(|e| Err(Into::into(e)), |_| { Ok(()) })
    }
    pub fn set_admin<'a, 'b, 'c, 'info>(
        ctx: anchor_lang::context::CpiContext<
            'a,
            'b,
            'c,
            'info,
            crate::cpi::accounts::SetAdmin<'info>,
        >,
    ) -> anchor_lang::Result<()> {
        let ix = {
            let ix = instruction::SetAdmin;
            let mut data = Vec::with_capacity(256);
            data.extend_from_slice(
                <instruction::SetAdmin as anchor_lang::Discriminator>::DISCRIMINATOR,
            );
            AnchorSerialize::serialize(&ix, &mut data)
                .map_err(|_| anchor_lang::error::ErrorCode::InstructionDidNotSerialize)?;
            let accounts = ctx.to_account_metas(None);
            anchor_lang::solana_program::instruction::Instruction {
                program_id: ctx.program.key(),
                accounts,
                data,
            }
        };
        let mut acc_infos = ctx.to_account_infos();
        anchor_lang::solana_program::program::invoke_signed(
                &ix,
                &acc_infos,
                ctx.signer_seeds,
            )
            .map_or_else(|e| Err(Into::into(e)), |_| { Ok(()) })
    }
    pub fn set_relayer<'a, 'b, 'c, 'info>(
        ctx: anchor_lang::context::CpiContext<
            'a,
            'b,
            'c,
            'info,
            crate::cpi::accounts::SetRelayer<'info>,
        >,
    ) -> anchor_lang::Result<()> {
        let ix = {
            let ix = instruction::SetRelayer;
            let mut data = Vec::with_capacity(256);
            data.extend_from_slice(
                <instruction::SetRelayer as anchor_lang::Discriminator>::DISCRIMINATOR,
            );
            AnchorSerialize::serialize(&ix, &mut data)
                .map_err(|_| anchor_lang::error::ErrorCode::InstructionDidNotSerialize)?;
            let accounts = ctx.to_account_metas(None);
            anchor_lang::solana_program::instruction::Instruction {
                program_id: ctx.program.key(),
                accounts,
                data,
            }
        };
        let mut acc_infos = ctx.to_account_infos();
        anchor_lang::solana_program::program::invoke_signed(
                &ix,
                &acc_infos,
                ctx.signer_seeds,
            )
            .map_or_else(|e| Err(Into::into(e)), |_| { Ok(()) })
    }
    pub fn set_splits<'a, 'b, 'c, 'info>(
        ctx: anchor_lang::context::CpiContext<
            'a,
            'b,
            'c,
            'info,
            crate::cpi::accounts::SetSplits<'info>,
        >,
        data: SetSplitsArgs,
    ) -> anchor_lang::Result<()> {
        let ix = {
            let ix = instruction::SetSplits { data };
            let mut data = Vec::with_capacity(256);
            data.extend_from_slice(
                <instruction::SetSplits as anchor_lang::Discriminator>::DISCRIMINATOR,
            );
            AnchorSerialize::serialize(&ix, &mut data)
                .map_err(|_| anchor_lang::error::ErrorCode::InstructionDidNotSerialize)?;
            let accounts = ctx.to_account_metas(None);
            anchor_lang::solana_program::instruction::Instruction {
                program_id: ctx.program.key(),
                accounts,
                data,
            }
        };
        let mut acc_infos = ctx.to_account_infos();
        anchor_lang::solana_program::program::invoke_signed(
                &ix,
                &acc_infos,
                ctx.signer_seeds,
            )
            .map_or_else(|e| Err(Into::into(e)), |_| { Ok(()) })
    }
    pub fn set_router_split<'a, 'b, 'c, 'info>(
        ctx: anchor_lang::context::CpiContext<
            'a,
            'b,
            'c,
            'info,
            crate::cpi::accounts::SetRouterSplit<'info>,
        >,
        data: SetRouterSplitArgs,
    ) -> anchor_lang::Result<()> {
        let ix = {
            let ix = instruction::SetRouterSplit {
                data,
            };
            let mut data = Vec::with_capacity(256);
            data.extend_from_slice(
                <instruction::SetRouterSplit as anchor_lang::Discriminator>::DISCRIMINATOR,
            );
            AnchorSerialize::serialize(&ix, &mut data)
                .map_err(|_| anchor_lang::error::ErrorCode::InstructionDidNotSerialize)?;
            let accounts = ctx.to_account_metas(None);
            anchor_lang::solana_program::instruction::Instruction {
                program_id: ctx.program.key(),
                accounts,
                data,
            }
        };
        let mut acc_infos = ctx.to_account_infos();
        anchor_lang::solana_program::program::invoke_signed(
                &ix,
                &acc_infos,
                ctx.signer_seeds,
            )
            .map_or_else(|e| Err(Into::into(e)), |_| { Ok(()) })
    }
    pub fn submit_bid<'a, 'b, 'c, 'info>(
        ctx: anchor_lang::context::CpiContext<
            'a,
            'b,
            'c,
            'info,
            crate::cpi::accounts::SubmitBid<'info>,
        >,
        data: SubmitBidArgs,
    ) -> anchor_lang::Result<()> {
        let ix = {
            let ix = instruction::SubmitBid { data };
            let mut data = Vec::with_capacity(256);
            data.extend_from_slice(
                <instruction::SubmitBid as anchor_lang::Discriminator>::DISCRIMINATOR,
            );
            AnchorSerialize::serialize(&ix, &mut data)
                .map_err(|_| anchor_lang::error::ErrorCode::InstructionDidNotSerialize)?;
            let accounts = ctx.to_account_metas(None);
            anchor_lang::solana_program::instruction::Instruction {
                program_id: ctx.program.key(),
                accounts,
                data,
            }
        };
        let mut acc_infos = ctx.to_account_infos();
        anchor_lang::solana_program::program::invoke_signed(
                &ix,
                &acc_infos,
                ctx.signer_seeds,
            )
            .map_or_else(|e| Err(Into::into(e)), |_| { Ok(()) })
    }
    pub fn check_permission<'a, 'b, 'c, 'info>(
        ctx: anchor_lang::context::CpiContext<
            'a,
            'b,
            'c,
            'info,
            crate::cpi::accounts::CheckPermission<'info>,
        >,
    ) -> anchor_lang::Result<crate::cpi::Return<u64>> {
        let ix = {
            let ix = instruction::CheckPermission;
            let mut data = Vec::with_capacity(256);
            data.extend_from_slice(
                <instruction::CheckPermission as anchor_lang::Discriminator>::DISCRIMINATOR,
            );
            AnchorSerialize::serialize(&ix, &mut data)
                .map_err(|_| anchor_lang::error::ErrorCode::InstructionDidNotSerialize)?;
            let accounts = ctx.to_account_metas(None);
            anchor_lang::solana_program::instruction::Instruction {
                program_id: ctx.program.key(),
                accounts,
                data,
            }
        };
        let mut acc_infos = ctx.to_account_infos();
        anchor_lang::solana_program::program::invoke_signed(
                &ix,
                &acc_infos,
                ctx.signer_seeds,
            )
            .map_or_else(
                |e| Err(Into::into(e)),
                |_| {
                    Ok(crate::cpi::Return::<u64> {
                        phantom: crate::cpi::PhantomData,
                    })
                },
            )
    }
    pub fn withdraw_fees<'a, 'b, 'c, 'info>(
        ctx: anchor_lang::context::CpiContext<
            'a,
            'b,
            'c,
            'info,
            crate::cpi::accounts::WithdrawFees<'info>,
        >,
    ) -> anchor_lang::Result<()> {
        let ix = {
            let ix = instruction::WithdrawFees;
            let mut data = Vec::with_capacity(256);
            data.extend_from_slice(
                <instruction::WithdrawFees as anchor_lang::Discriminator>::DISCRIMINATOR,
            );
            AnchorSerialize::serialize(&ix, &mut data)
                .map_err(|_| anchor_lang::error::ErrorCode::InstructionDidNotSerialize)?;
            let accounts = ctx.to_account_metas(None);
            anchor_lang::solana_program::instruction::Instruction {
                program_id: ctx.program.key(),
                accounts,
                data,
            }
        };
        let mut acc_infos = ctx.to_account_infos();
        anchor_lang::solana_program::program::invoke_signed(
                &ix,
                &acc_infos,
                ctx.signer_seeds,
            )
            .map_or_else(|e| Err(Into::into(e)), |_| { Ok(()) })
    }
    pub fn swap<'a, 'b, 'c, 'info>(
        ctx: anchor_lang::context::CpiContext<
            'a,
            'b,
            'c,
            'info,
            crate::cpi::accounts::Swap<'info>,
        >,
        data: SwapArgs,
    ) -> anchor_lang::Result<()> {
        let ix = {
            let ix = instruction::Swap { data };
            let mut data = Vec::with_capacity(256);
            data.extend_from_slice(
                <instruction::Swap as anchor_lang::Discriminator>::DISCRIMINATOR,
            );
            AnchorSerialize::serialize(&ix, &mut data)
                .map_err(|_| anchor_lang::error::ErrorCode::InstructionDidNotSerialize)?;
            let accounts = ctx.to_account_metas(None);
            anchor_lang::solana_program::instruction::Instruction {
                program_id: ctx.program.key(),
                accounts,
                data,
            }
        };
        let mut acc_infos = ctx.to_account_infos();
        anchor_lang::solana_program::program::invoke_signed(
                &ix,
                &acc_infos,
                ctx.signer_seeds,
            )
            .map_or_else(|e| Err(Into::into(e)), |_| { Ok(()) })
    }
    /// An Anchor generated module, providing a set of structs
    /// mirroring the structs deriving `Accounts`, where each field is
    /// an `AccountInfo`. This is useful for CPI.
    pub mod accounts {
        pub use crate::__cpi_client_accounts_set_relayer::*;
        pub use crate::__cpi_client_accounts_set_admin::*;
        pub use crate::__cpi_client_accounts_set_router_split::*;
        pub use crate::__cpi_client_accounts_withdraw_fees::*;
        pub use crate::__cpi_client_accounts_check_permission::*;
        pub use crate::__cpi_client_accounts_submit_bid::*;
        pub use crate::__cpi_client_accounts_set_splits::*;
        pub use crate::__cpi_client_accounts_initialize::*;
        pub use crate::__cpi_client_accounts_swap::*;
    }
}
/// An Anchor generated module, providing a set of structs
/// mirroring the structs deriving `Accounts`, where each field is
/// a `Pubkey`. This is useful for specifying accounts for a client.
pub mod accounts {
    pub use crate::__client_accounts_initialize::*;
    pub use crate::__client_accounts_set_relayer::*;
    pub use crate::__client_accounts_submit_bid::*;
    pub use crate::__client_accounts_swap::*;
    pub use crate::__client_accounts_check_permission::*;
    pub use crate::__client_accounts_set_admin::*;
    pub use crate::__client_accounts_set_router_split::*;
    pub use crate::__client_accounts_set_splits::*;
    pub use crate::__client_accounts_withdraw_fees::*;
}
pub struct InitializeArgs {
    pub split_router_default: u64,
    pub split_relayer: u64,
}
impl borsh::ser::BorshSerialize for InitializeArgs
where
    u64: borsh::ser::BorshSerialize,
    u64: borsh::ser::BorshSerialize,
{
    fn serialize<W: borsh::maybestd::io::Write>(
        &self,
        writer: &mut W,
    ) -> ::core::result::Result<(), borsh::maybestd::io::Error> {
        borsh::BorshSerialize::serialize(&self.split_router_default, writer)?;
        borsh::BorshSerialize::serialize(&self.split_relayer, writer)?;
        Ok(())
    }
}
impl borsh::de::BorshDeserialize for InitializeArgs
where
    u64: borsh::BorshDeserialize,
    u64: borsh::BorshDeserialize,
{
    fn deserialize_reader<R: borsh::maybestd::io::Read>(
        reader: &mut R,
    ) -> ::core::result::Result<Self, borsh::maybestd::io::Error> {
        Ok(Self {
            split_router_default: borsh::BorshDeserialize::deserialize_reader(reader)?,
            split_relayer: borsh::BorshDeserialize::deserialize_reader(reader)?,
        })
    }
}
#[automatically_derived]
impl ::core::cmp::Eq for InitializeArgs {
    #[inline]
    #[doc(hidden)]
    #[coverage(off)]
    fn assert_receiver_is_total_eq(&self) -> () {
        let _: ::core::cmp::AssertParamIsEq<u64>;
    }
}
#[automatically_derived]
impl ::core::marker::StructuralPartialEq for InitializeArgs {}
#[automatically_derived]
impl ::core::cmp::PartialEq for InitializeArgs {
    #[inline]
    fn eq(&self, other: &InitializeArgs) -> bool {
        self.split_router_default == other.split_router_default
            && self.split_relayer == other.split_relayer
    }
}
#[automatically_derived]
impl ::core::clone::Clone for InitializeArgs {
    #[inline]
    fn clone(&self) -> InitializeArgs {
        let _: ::core::clone::AssertParamIsClone<u64>;
        *self
    }
}
#[automatically_derived]
impl ::core::marker::Copy for InitializeArgs {}
#[automatically_derived]
impl ::core::fmt::Debug for InitializeArgs {
    #[inline]
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        ::core::fmt::Formatter::debug_struct_field2_finish(
            f,
            "InitializeArgs",
            "split_router_default",
            &self.split_router_default,
            "split_relayer",
            &&self.split_relayer,
        )
    }
}
pub struct Initialize<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(
        init,
        payer = payer,
        space = RESERVE_EXPRESS_RELAY_METADATA,
        seeds = [SEED_METADATA],
        bump
    )]
    pub express_relay_metadata: Account<'info, ExpressRelayMetadata>,
    /// CHECK: this is just the admin's PK.
    pub admin: UncheckedAccount<'info>,
    /// CHECK: this is just the relayer's signer PK.
    pub relayer_signer: UncheckedAccount<'info>,
    /// CHECK: this is just a PK for the relayer to receive fees at.
    pub fee_receiver_relayer: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}
#[automatically_derived]
impl<'info> anchor_lang::Accounts<'info, InitializeBumps> for Initialize<'info>
where
    'info: 'info,
{
    #[inline(never)]
    fn try_accounts(
        __program_id: &anchor_lang::solana_program::pubkey::Pubkey,
        __accounts: &mut &'info [anchor_lang::solana_program::account_info::AccountInfo<
            'info,
        >],
        __ix_data: &[u8],
        __bumps: &mut InitializeBumps,
        __reallocs: &mut std::collections::BTreeSet<
            anchor_lang::solana_program::pubkey::Pubkey,
        >,
    ) -> anchor_lang::Result<Self> {
        let payer: Signer = anchor_lang::Accounts::try_accounts(
                __program_id,
                __accounts,
                __ix_data,
                __bumps,
                __reallocs,
            )
            .map_err(|e| e.with_account_name("payer"))?;
        if __accounts.is_empty() {
            return Err(anchor_lang::error::ErrorCode::AccountNotEnoughKeys.into());
        }
        let express_relay_metadata = &__accounts[0];
        *__accounts = &__accounts[1..];
        let admin: UncheckedAccount = anchor_lang::Accounts::try_accounts(
                __program_id,
                __accounts,
                __ix_data,
                __bumps,
                __reallocs,
            )
            .map_err(|e| e.with_account_name("admin"))?;
        let relayer_signer: UncheckedAccount = anchor_lang::Accounts::try_accounts(
                __program_id,
                __accounts,
                __ix_data,
                __bumps,
                __reallocs,
            )
            .map_err(|e| e.with_account_name("relayer_signer"))?;
        let fee_receiver_relayer: UncheckedAccount = anchor_lang::Accounts::try_accounts(
                __program_id,
                __accounts,
                __ix_data,
                __bumps,
                __reallocs,
            )
            .map_err(|e| e.with_account_name("fee_receiver_relayer"))?;
        let system_program: anchor_lang::accounts::program::Program<System> = anchor_lang::Accounts::try_accounts(
                __program_id,
                __accounts,
                __ix_data,
                __bumps,
                __reallocs,
            )
            .map_err(|e| e.with_account_name("system_program"))?;
        let __anchor_rent = Rent::get()?;
        let (__pda_address, __bump) = Pubkey::find_program_address(
            &[SEED_METADATA],
            __program_id,
        );
        __bumps.express_relay_metadata = __bump;
        if express_relay_metadata.key() != __pda_address {
            return Err(
                anchor_lang::error::Error::from(
                        anchor_lang::error::ErrorCode::ConstraintSeeds,
                    )
                    .with_account_name("express_relay_metadata")
                    .with_pubkeys((express_relay_metadata.key(), __pda_address)),
            );
        }
        let express_relay_metadata = ({
            #[inline(never)]
            || {
                let actual_field = AsRef::<AccountInfo>::as_ref(&express_relay_metadata);
                let actual_owner = actual_field.owner;
                let space = RESERVE_EXPRESS_RELAY_METADATA;
                let pa: anchor_lang::accounts::account::Account<ExpressRelayMetadata> = if !false
                    || actual_owner == &anchor_lang::solana_program::system_program::ID
                {
                    let __current_lamports = express_relay_metadata.lamports();
                    if __current_lamports == 0 {
                        let space = space;
                        let lamports = __anchor_rent.minimum_balance(space);
                        let cpi_accounts = anchor_lang::system_program::CreateAccount {
                            from: payer.to_account_info(),
                            to: express_relay_metadata.to_account_info(),
                        };
                        let cpi_context = anchor_lang::context::CpiContext::new(
                            system_program.to_account_info(),
                            cpi_accounts,
                        );
                        anchor_lang::system_program::create_account(
                            cpi_context
                                .with_signer(&[&[SEED_METADATA, &[__bump][..]][..]]),
                            lamports,
                            space as u64,
                            __program_id,
                        )?;
                    } else {
                        if payer.key() == express_relay_metadata.key() {
                            return Err(
                                anchor_lang::error::Error::from(anchor_lang::error::AnchorError {
                                        error_name: anchor_lang::error::ErrorCode::TryingToInitPayerAsProgramAccount
                                            .name(),
                                        error_code_number: anchor_lang::error::ErrorCode::TryingToInitPayerAsProgramAccount
                                            .into(),
                                        error_msg: anchor_lang::error::ErrorCode::TryingToInitPayerAsProgramAccount
                                            .to_string(),
                                        error_origin: Some(
                                            anchor_lang::error::ErrorOrigin::Source(anchor_lang::error::Source {
                                                filename: "programs/express_relay/src/lib.rs",
                                                line: 267u32,
                                            }),
                                        ),
                                        compared_values: None,
                                    })
                                    .with_pubkeys((payer.key(), express_relay_metadata.key())),
                            );
                        }
                        let required_lamports = __anchor_rent
                            .minimum_balance(space)
                            .max(1)
                            .saturating_sub(__current_lamports);
                        if required_lamports > 0 {
                            let cpi_accounts = anchor_lang::system_program::Transfer {
                                from: payer.to_account_info(),
                                to: express_relay_metadata.to_account_info(),
                            };
                            let cpi_context = anchor_lang::context::CpiContext::new(
                                system_program.to_account_info(),
                                cpi_accounts,
                            );
                            anchor_lang::system_program::transfer(
                                cpi_context,
                                required_lamports,
                            )?;
                        }
                        let cpi_accounts = anchor_lang::system_program::Allocate {
                            account_to_allocate: express_relay_metadata.to_account_info(),
                        };
                        let cpi_context = anchor_lang::context::CpiContext::new(
                            system_program.to_account_info(),
                            cpi_accounts,
                        );
                        anchor_lang::system_program::allocate(
                            cpi_context
                                .with_signer(&[&[SEED_METADATA, &[__bump][..]][..]]),
                            space as u64,
                        )?;
                        let cpi_accounts = anchor_lang::system_program::Assign {
                            account_to_assign: express_relay_metadata.to_account_info(),
                        };
                        let cpi_context = anchor_lang::context::CpiContext::new(
                            system_program.to_account_info(),
                            cpi_accounts,
                        );
                        anchor_lang::system_program::assign(
                            cpi_context
                                .with_signer(&[&[SEED_METADATA, &[__bump][..]][..]]),
                            __program_id,
                        )?;
                    }
                    match anchor_lang::accounts::account::Account::try_from_unchecked(
                        &express_relay_metadata,
                    ) {
                        Ok(val) => val,
                        Err(e) => {
                            return Err(e.with_account_name("express_relay_metadata"));
                        }
                    }
                } else {
                    match anchor_lang::accounts::account::Account::try_from(
                        &express_relay_metadata,
                    ) {
                        Ok(val) => val,
                        Err(e) => {
                            return Err(e.with_account_name("express_relay_metadata"));
                        }
                    }
                };
                if false {
                    if space != actual_field.data_len() {
                        return Err(
                            anchor_lang::error::Error::from(
                                    anchor_lang::error::ErrorCode::ConstraintSpace,
                                )
                                .with_account_name("express_relay_metadata")
                                .with_values((space, actual_field.data_len())),
                        );
                    }
                    if actual_owner != __program_id {
                        return Err(
                            anchor_lang::error::Error::from(
                                    anchor_lang::error::ErrorCode::ConstraintOwner,
                                )
                                .with_account_name("express_relay_metadata")
                                .with_pubkeys((*actual_owner, *__program_id)),
                        );
                    }
                    {
                        let required_lamports = __anchor_rent.minimum_balance(space);
                        if pa.to_account_info().lamports() < required_lamports {
                            return Err(
                                anchor_lang::error::Error::from(
                                        anchor_lang::error::ErrorCode::ConstraintRentExempt,
                                    )
                                    .with_account_name("express_relay_metadata"),
                            );
                        }
                    }
                }
                Ok(pa)
            }
        })()?;
        if !AsRef::<AccountInfo>::as_ref(&express_relay_metadata).is_writable {
            return Err(
                anchor_lang::error::Error::from(
                        anchor_lang::error::ErrorCode::ConstraintMut,
                    )
                    .with_account_name("express_relay_metadata"),
            );
        }
        if !__anchor_rent
            .is_exempt(
                express_relay_metadata.to_account_info().lamports(),
                express_relay_metadata.to_account_info().try_data_len()?,
            )
        {
            return Err(
                anchor_lang::error::Error::from(
                        anchor_lang::error::ErrorCode::ConstraintRentExempt,
                    )
                    .with_account_name("express_relay_metadata"),
            );
        }
        if !AsRef::<AccountInfo>::as_ref(&payer).is_writable {
            return Err(
                anchor_lang::error::Error::from(
                        anchor_lang::error::ErrorCode::ConstraintMut,
                    )
                    .with_account_name("payer"),
            );
        }
        Ok(Initialize {
            payer,
            express_relay_metadata,
            admin,
            relayer_signer,
            fee_receiver_relayer,
            system_program,
        })
    }
}
#[automatically_derived]
impl<'info> anchor_lang::ToAccountInfos<'info> for Initialize<'info>
where
    'info: 'info,
{
    fn to_account_infos(
        &self,
    ) -> Vec<anchor_lang::solana_program::account_info::AccountInfo<'info>> {
        let mut account_infos = ::alloc::vec::Vec::new();
        account_infos.extend(self.payer.to_account_infos());
        account_infos.extend(self.express_relay_metadata.to_account_infos());
        account_infos.extend(self.admin.to_account_infos());
        account_infos.extend(self.relayer_signer.to_account_infos());
        account_infos.extend(self.fee_receiver_relayer.to_account_infos());
        account_infos.extend(self.system_program.to_account_infos());
        account_infos
    }
}
#[automatically_derived]
impl<'info> anchor_lang::ToAccountMetas for Initialize<'info> {
    fn to_account_metas(
        &self,
        is_signer: Option<bool>,
    ) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
        let mut account_metas = ::alloc::vec::Vec::new();
        account_metas.extend(self.payer.to_account_metas(None));
        account_metas.extend(self.express_relay_metadata.to_account_metas(None));
        account_metas.extend(self.admin.to_account_metas(None));
        account_metas.extend(self.relayer_signer.to_account_metas(None));
        account_metas.extend(self.fee_receiver_relayer.to_account_metas(None));
        account_metas.extend(self.system_program.to_account_metas(None));
        account_metas
    }
}
#[automatically_derived]
impl<'info> anchor_lang::AccountsExit<'info> for Initialize<'info>
where
    'info: 'info,
{
    fn exit(
        &self,
        program_id: &anchor_lang::solana_program::pubkey::Pubkey,
    ) -> anchor_lang::Result<()> {
        anchor_lang::AccountsExit::exit(&self.payer, program_id)
            .map_err(|e| e.with_account_name("payer"))?;
        anchor_lang::AccountsExit::exit(&self.express_relay_metadata, program_id)
            .map_err(|e| e.with_account_name("express_relay_metadata"))?;
        Ok(())
    }
}
pub struct InitializeBumps {
    pub express_relay_metadata: u8,
}
#[automatically_derived]
impl ::core::fmt::Debug for InitializeBumps {
    #[inline]
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        ::core::fmt::Formatter::debug_struct_field1_finish(
            f,
            "InitializeBumps",
            "express_relay_metadata",
            &&self.express_relay_metadata,
        )
    }
}
impl Default for InitializeBumps {
    fn default() -> Self {
        InitializeBumps {
            express_relay_metadata: u8::MAX,
        }
    }
}
impl<'info> anchor_lang::Bumps for Initialize<'info>
where
    'info: 'info,
{
    type Bumps = InitializeBumps;
}
/// An internal, Anchor generated module. This is used (as an
/// implementation detail), to generate a struct for a given
/// `#[derive(Accounts)]` implementation, where each field is a Pubkey,
/// instead of an `AccountInfo`. This is useful for clients that want
/// to generate a list of accounts, without explicitly knowing the
/// order all the fields should be in.
///
/// To access the struct in this module, one should use the sibling
/// `accounts` module (also generated), which re-exports this.
pub(crate) mod __client_accounts_initialize {
    use super::*;
    use anchor_lang::prelude::borsh;
    /// Generated client accounts for [`Initialize`].
    pub struct Initialize {
        pub payer: Pubkey,
        pub express_relay_metadata: Pubkey,
        pub admin: Pubkey,
        pub relayer_signer: Pubkey,
        pub fee_receiver_relayer: Pubkey,
        pub system_program: Pubkey,
    }
    impl borsh::ser::BorshSerialize for Initialize
    where
        Pubkey: borsh::ser::BorshSerialize,
        Pubkey: borsh::ser::BorshSerialize,
        Pubkey: borsh::ser::BorshSerialize,
        Pubkey: borsh::ser::BorshSerialize,
        Pubkey: borsh::ser::BorshSerialize,
        Pubkey: borsh::ser::BorshSerialize,
    {
        fn serialize<W: borsh::maybestd::io::Write>(
            &self,
            writer: &mut W,
        ) -> ::core::result::Result<(), borsh::maybestd::io::Error> {
            borsh::BorshSerialize::serialize(&self.payer, writer)?;
            borsh::BorshSerialize::serialize(&self.express_relay_metadata, writer)?;
            borsh::BorshSerialize::serialize(&self.admin, writer)?;
            borsh::BorshSerialize::serialize(&self.relayer_signer, writer)?;
            borsh::BorshSerialize::serialize(&self.fee_receiver_relayer, writer)?;
            borsh::BorshSerialize::serialize(&self.system_program, writer)?;
            Ok(())
        }
    }
    #[automatically_derived]
    impl anchor_lang::ToAccountMetas for Initialize {
        fn to_account_metas(
            &self,
            is_signer: Option<bool>,
        ) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
            let mut account_metas = ::alloc::vec::Vec::new();
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        self.payer,
                        true,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        self.express_relay_metadata,
                        false,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        self.admin,
                        false,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        self.relayer_signer,
                        false,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        self.fee_receiver_relayer,
                        false,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        self.system_program,
                        false,
                    ),
                );
            account_metas
        }
    }
}
/// An internal, Anchor generated module. This is used (as an
/// implementation detail), to generate a CPI struct for a given
/// `#[derive(Accounts)]` implementation, where each field is an
/// AccountInfo.
///
/// To access the struct in this module, one should use the sibling
/// [`cpi::accounts`] module (also generated), which re-exports this.
pub(crate) mod __cpi_client_accounts_initialize {
    use super::*;
    /// Generated CPI struct of the accounts for [`Initialize`].
    pub struct Initialize<'info> {
        pub payer: anchor_lang::solana_program::account_info::AccountInfo<'info>,
        pub express_relay_metadata: anchor_lang::solana_program::account_info::AccountInfo<
            'info,
        >,
        pub admin: anchor_lang::solana_program::account_info::AccountInfo<'info>,
        pub relayer_signer: anchor_lang::solana_program::account_info::AccountInfo<
            'info,
        >,
        pub fee_receiver_relayer: anchor_lang::solana_program::account_info::AccountInfo<
            'info,
        >,
        pub system_program: anchor_lang::solana_program::account_info::AccountInfo<
            'info,
        >,
    }
    #[automatically_derived]
    impl<'info> anchor_lang::ToAccountMetas for Initialize<'info> {
        fn to_account_metas(
            &self,
            is_signer: Option<bool>,
        ) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
            let mut account_metas = ::alloc::vec::Vec::new();
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        anchor_lang::Key::key(&self.payer),
                        true,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        anchor_lang::Key::key(&self.express_relay_metadata),
                        false,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        anchor_lang::Key::key(&self.admin),
                        false,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        anchor_lang::Key::key(&self.relayer_signer),
                        false,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        anchor_lang::Key::key(&self.fee_receiver_relayer),
                        false,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        anchor_lang::Key::key(&self.system_program),
                        false,
                    ),
                );
            account_metas
        }
    }
    #[automatically_derived]
    impl<'info> anchor_lang::ToAccountInfos<'info> for Initialize<'info> {
        fn to_account_infos(
            &self,
        ) -> Vec<anchor_lang::solana_program::account_info::AccountInfo<'info>> {
            let mut account_infos = ::alloc::vec::Vec::new();
            account_infos
                .extend(anchor_lang::ToAccountInfos::to_account_infos(&self.payer));
            account_infos
                .extend(
                    anchor_lang::ToAccountInfos::to_account_infos(
                        &self.express_relay_metadata,
                    ),
                );
            account_infos
                .extend(anchor_lang::ToAccountInfos::to_account_infos(&self.admin));
            account_infos
                .extend(
                    anchor_lang::ToAccountInfos::to_account_infos(&self.relayer_signer),
                );
            account_infos
                .extend(
                    anchor_lang::ToAccountInfos::to_account_infos(
                        &self.fee_receiver_relayer,
                    ),
                );
            account_infos
                .extend(
                    anchor_lang::ToAccountInfos::to_account_infos(&self.system_program),
                );
            account_infos
        }
    }
}
pub struct SetAdmin<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,
    #[account(mut, seeds = [SEED_METADATA], bump, has_one = admin)]
    pub express_relay_metadata: Account<'info, ExpressRelayMetadata>,
    /// CHECK: this is just the new admin PK.
    pub admin_new: UncheckedAccount<'info>,
}
#[automatically_derived]
impl<'info> anchor_lang::Accounts<'info, SetAdminBumps> for SetAdmin<'info>
where
    'info: 'info,
{
    #[inline(never)]
    fn try_accounts(
        __program_id: &anchor_lang::solana_program::pubkey::Pubkey,
        __accounts: &mut &'info [anchor_lang::solana_program::account_info::AccountInfo<
            'info,
        >],
        __ix_data: &[u8],
        __bumps: &mut SetAdminBumps,
        __reallocs: &mut std::collections::BTreeSet<
            anchor_lang::solana_program::pubkey::Pubkey,
        >,
    ) -> anchor_lang::Result<Self> {
        let admin: Signer = anchor_lang::Accounts::try_accounts(
                __program_id,
                __accounts,
                __ix_data,
                __bumps,
                __reallocs,
            )
            .map_err(|e| e.with_account_name("admin"))?;
        let express_relay_metadata: anchor_lang::accounts::account::Account<
            ExpressRelayMetadata,
        > = anchor_lang::Accounts::try_accounts(
                __program_id,
                __accounts,
                __ix_data,
                __bumps,
                __reallocs,
            )
            .map_err(|e| e.with_account_name("express_relay_metadata"))?;
        let admin_new: UncheckedAccount = anchor_lang::Accounts::try_accounts(
                __program_id,
                __accounts,
                __ix_data,
                __bumps,
                __reallocs,
            )
            .map_err(|e| e.with_account_name("admin_new"))?;
        if !AsRef::<AccountInfo>::as_ref(&admin).is_writable {
            return Err(
                anchor_lang::error::Error::from(
                        anchor_lang::error::ErrorCode::ConstraintMut,
                    )
                    .with_account_name("admin"),
            );
        }
        let (__pda_address, __bump) = Pubkey::find_program_address(
            &[SEED_METADATA],
            &__program_id,
        );
        __bumps.express_relay_metadata = __bump;
        if express_relay_metadata.key() != __pda_address {
            return Err(
                anchor_lang::error::Error::from(
                        anchor_lang::error::ErrorCode::ConstraintSeeds,
                    )
                    .with_account_name("express_relay_metadata")
                    .with_pubkeys((express_relay_metadata.key(), __pda_address)),
            );
        }
        if !AsRef::<AccountInfo>::as_ref(&express_relay_metadata).is_writable {
            return Err(
                anchor_lang::error::Error::from(
                        anchor_lang::error::ErrorCode::ConstraintMut,
                    )
                    .with_account_name("express_relay_metadata"),
            );
        }
        {
            let my_key = express_relay_metadata.admin;
            let target_key = admin.key();
            if my_key != target_key {
                return Err(
                    anchor_lang::error::Error::from(
                            anchor_lang::error::ErrorCode::ConstraintHasOne,
                        )
                        .with_account_name("express_relay_metadata")
                        .with_pubkeys((my_key, target_key)),
                );
            }
        }
        Ok(SetAdmin {
            admin,
            express_relay_metadata,
            admin_new,
        })
    }
}
#[automatically_derived]
impl<'info> anchor_lang::ToAccountInfos<'info> for SetAdmin<'info>
where
    'info: 'info,
{
    fn to_account_infos(
        &self,
    ) -> Vec<anchor_lang::solana_program::account_info::AccountInfo<'info>> {
        let mut account_infos = ::alloc::vec::Vec::new();
        account_infos.extend(self.admin.to_account_infos());
        account_infos.extend(self.express_relay_metadata.to_account_infos());
        account_infos.extend(self.admin_new.to_account_infos());
        account_infos
    }
}
#[automatically_derived]
impl<'info> anchor_lang::ToAccountMetas for SetAdmin<'info> {
    fn to_account_metas(
        &self,
        is_signer: Option<bool>,
    ) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
        let mut account_metas = ::alloc::vec::Vec::new();
        account_metas.extend(self.admin.to_account_metas(None));
        account_metas.extend(self.express_relay_metadata.to_account_metas(None));
        account_metas.extend(self.admin_new.to_account_metas(None));
        account_metas
    }
}
#[automatically_derived]
impl<'info> anchor_lang::AccountsExit<'info> for SetAdmin<'info>
where
    'info: 'info,
{
    fn exit(
        &self,
        program_id: &anchor_lang::solana_program::pubkey::Pubkey,
    ) -> anchor_lang::Result<()> {
        anchor_lang::AccountsExit::exit(&self.admin, program_id)
            .map_err(|e| e.with_account_name("admin"))?;
        anchor_lang::AccountsExit::exit(&self.express_relay_metadata, program_id)
            .map_err(|e| e.with_account_name("express_relay_metadata"))?;
        Ok(())
    }
}
pub struct SetAdminBumps {
    pub express_relay_metadata: u8,
}
#[automatically_derived]
impl ::core::fmt::Debug for SetAdminBumps {
    #[inline]
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        ::core::fmt::Formatter::debug_struct_field1_finish(
            f,
            "SetAdminBumps",
            "express_relay_metadata",
            &&self.express_relay_metadata,
        )
    }
}
impl Default for SetAdminBumps {
    fn default() -> Self {
        SetAdminBumps {
            express_relay_metadata: u8::MAX,
        }
    }
}
impl<'info> anchor_lang::Bumps for SetAdmin<'info>
where
    'info: 'info,
{
    type Bumps = SetAdminBumps;
}
/// An internal, Anchor generated module. This is used (as an
/// implementation detail), to generate a struct for a given
/// `#[derive(Accounts)]` implementation, where each field is a Pubkey,
/// instead of an `AccountInfo`. This is useful for clients that want
/// to generate a list of accounts, without explicitly knowing the
/// order all the fields should be in.
///
/// To access the struct in this module, one should use the sibling
/// `accounts` module (also generated), which re-exports this.
pub(crate) mod __client_accounts_set_admin {
    use super::*;
    use anchor_lang::prelude::borsh;
    /// Generated client accounts for [`SetAdmin`].
    pub struct SetAdmin {
        pub admin: Pubkey,
        pub express_relay_metadata: Pubkey,
        pub admin_new: Pubkey,
    }
    impl borsh::ser::BorshSerialize for SetAdmin
    where
        Pubkey: borsh::ser::BorshSerialize,
        Pubkey: borsh::ser::BorshSerialize,
        Pubkey: borsh::ser::BorshSerialize,
    {
        fn serialize<W: borsh::maybestd::io::Write>(
            &self,
            writer: &mut W,
        ) -> ::core::result::Result<(), borsh::maybestd::io::Error> {
            borsh::BorshSerialize::serialize(&self.admin, writer)?;
            borsh::BorshSerialize::serialize(&self.express_relay_metadata, writer)?;
            borsh::BorshSerialize::serialize(&self.admin_new, writer)?;
            Ok(())
        }
    }
    #[automatically_derived]
    impl anchor_lang::ToAccountMetas for SetAdmin {
        fn to_account_metas(
            &self,
            is_signer: Option<bool>,
        ) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
            let mut account_metas = ::alloc::vec::Vec::new();
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        self.admin,
                        true,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        self.express_relay_metadata,
                        false,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        self.admin_new,
                        false,
                    ),
                );
            account_metas
        }
    }
}
/// An internal, Anchor generated module. This is used (as an
/// implementation detail), to generate a CPI struct for a given
/// `#[derive(Accounts)]` implementation, where each field is an
/// AccountInfo.
///
/// To access the struct in this module, one should use the sibling
/// [`cpi::accounts`] module (also generated), which re-exports this.
pub(crate) mod __cpi_client_accounts_set_admin {
    use super::*;
    /// Generated CPI struct of the accounts for [`SetAdmin`].
    pub struct SetAdmin<'info> {
        pub admin: anchor_lang::solana_program::account_info::AccountInfo<'info>,
        pub express_relay_metadata: anchor_lang::solana_program::account_info::AccountInfo<
            'info,
        >,
        pub admin_new: anchor_lang::solana_program::account_info::AccountInfo<'info>,
    }
    #[automatically_derived]
    impl<'info> anchor_lang::ToAccountMetas for SetAdmin<'info> {
        fn to_account_metas(
            &self,
            is_signer: Option<bool>,
        ) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
            let mut account_metas = ::alloc::vec::Vec::new();
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        anchor_lang::Key::key(&self.admin),
                        true,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        anchor_lang::Key::key(&self.express_relay_metadata),
                        false,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        anchor_lang::Key::key(&self.admin_new),
                        false,
                    ),
                );
            account_metas
        }
    }
    #[automatically_derived]
    impl<'info> anchor_lang::ToAccountInfos<'info> for SetAdmin<'info> {
        fn to_account_infos(
            &self,
        ) -> Vec<anchor_lang::solana_program::account_info::AccountInfo<'info>> {
            let mut account_infos = ::alloc::vec::Vec::new();
            account_infos
                .extend(anchor_lang::ToAccountInfos::to_account_infos(&self.admin));
            account_infos
                .extend(
                    anchor_lang::ToAccountInfos::to_account_infos(
                        &self.express_relay_metadata,
                    ),
                );
            account_infos
                .extend(anchor_lang::ToAccountInfos::to_account_infos(&self.admin_new));
            account_infos
        }
    }
}
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
#[automatically_derived]
impl<'info> anchor_lang::Accounts<'info, SetRelayerBumps> for SetRelayer<'info>
where
    'info: 'info,
{
    #[inline(never)]
    fn try_accounts(
        __program_id: &anchor_lang::solana_program::pubkey::Pubkey,
        __accounts: &mut &'info [anchor_lang::solana_program::account_info::AccountInfo<
            'info,
        >],
        __ix_data: &[u8],
        __bumps: &mut SetRelayerBumps,
        __reallocs: &mut std::collections::BTreeSet<
            anchor_lang::solana_program::pubkey::Pubkey,
        >,
    ) -> anchor_lang::Result<Self> {
        let admin: Signer = anchor_lang::Accounts::try_accounts(
                __program_id,
                __accounts,
                __ix_data,
                __bumps,
                __reallocs,
            )
            .map_err(|e| e.with_account_name("admin"))?;
        let express_relay_metadata: anchor_lang::accounts::account::Account<
            ExpressRelayMetadata,
        > = anchor_lang::Accounts::try_accounts(
                __program_id,
                __accounts,
                __ix_data,
                __bumps,
                __reallocs,
            )
            .map_err(|e| e.with_account_name("express_relay_metadata"))?;
        let relayer_signer: UncheckedAccount = anchor_lang::Accounts::try_accounts(
                __program_id,
                __accounts,
                __ix_data,
                __bumps,
                __reallocs,
            )
            .map_err(|e| e.with_account_name("relayer_signer"))?;
        let fee_receiver_relayer: UncheckedAccount = anchor_lang::Accounts::try_accounts(
                __program_id,
                __accounts,
                __ix_data,
                __bumps,
                __reallocs,
            )
            .map_err(|e| e.with_account_name("fee_receiver_relayer"))?;
        if !AsRef::<AccountInfo>::as_ref(&admin).is_writable {
            return Err(
                anchor_lang::error::Error::from(
                        anchor_lang::error::ErrorCode::ConstraintMut,
                    )
                    .with_account_name("admin"),
            );
        }
        let (__pda_address, __bump) = Pubkey::find_program_address(
            &[SEED_METADATA],
            &__program_id,
        );
        __bumps.express_relay_metadata = __bump;
        if express_relay_metadata.key() != __pda_address {
            return Err(
                anchor_lang::error::Error::from(
                        anchor_lang::error::ErrorCode::ConstraintSeeds,
                    )
                    .with_account_name("express_relay_metadata")
                    .with_pubkeys((express_relay_metadata.key(), __pda_address)),
            );
        }
        if !AsRef::<AccountInfo>::as_ref(&express_relay_metadata).is_writable {
            return Err(
                anchor_lang::error::Error::from(
                        anchor_lang::error::ErrorCode::ConstraintMut,
                    )
                    .with_account_name("express_relay_metadata"),
            );
        }
        {
            let my_key = express_relay_metadata.admin;
            let target_key = admin.key();
            if my_key != target_key {
                return Err(
                    anchor_lang::error::Error::from(
                            anchor_lang::error::ErrorCode::ConstraintHasOne,
                        )
                        .with_account_name("express_relay_metadata")
                        .with_pubkeys((my_key, target_key)),
                );
            }
        }
        Ok(SetRelayer {
            admin,
            express_relay_metadata,
            relayer_signer,
            fee_receiver_relayer,
        })
    }
}
#[automatically_derived]
impl<'info> anchor_lang::ToAccountInfos<'info> for SetRelayer<'info>
where
    'info: 'info,
{
    fn to_account_infos(
        &self,
    ) -> Vec<anchor_lang::solana_program::account_info::AccountInfo<'info>> {
        let mut account_infos = ::alloc::vec::Vec::new();
        account_infos.extend(self.admin.to_account_infos());
        account_infos.extend(self.express_relay_metadata.to_account_infos());
        account_infos.extend(self.relayer_signer.to_account_infos());
        account_infos.extend(self.fee_receiver_relayer.to_account_infos());
        account_infos
    }
}
#[automatically_derived]
impl<'info> anchor_lang::ToAccountMetas for SetRelayer<'info> {
    fn to_account_metas(
        &self,
        is_signer: Option<bool>,
    ) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
        let mut account_metas = ::alloc::vec::Vec::new();
        account_metas.extend(self.admin.to_account_metas(None));
        account_metas.extend(self.express_relay_metadata.to_account_metas(None));
        account_metas.extend(self.relayer_signer.to_account_metas(None));
        account_metas.extend(self.fee_receiver_relayer.to_account_metas(None));
        account_metas
    }
}
#[automatically_derived]
impl<'info> anchor_lang::AccountsExit<'info> for SetRelayer<'info>
where
    'info: 'info,
{
    fn exit(
        &self,
        program_id: &anchor_lang::solana_program::pubkey::Pubkey,
    ) -> anchor_lang::Result<()> {
        anchor_lang::AccountsExit::exit(&self.admin, program_id)
            .map_err(|e| e.with_account_name("admin"))?;
        anchor_lang::AccountsExit::exit(&self.express_relay_metadata, program_id)
            .map_err(|e| e.with_account_name("express_relay_metadata"))?;
        Ok(())
    }
}
pub struct SetRelayerBumps {
    pub express_relay_metadata: u8,
}
#[automatically_derived]
impl ::core::fmt::Debug for SetRelayerBumps {
    #[inline]
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        ::core::fmt::Formatter::debug_struct_field1_finish(
            f,
            "SetRelayerBumps",
            "express_relay_metadata",
            &&self.express_relay_metadata,
        )
    }
}
impl Default for SetRelayerBumps {
    fn default() -> Self {
        SetRelayerBumps {
            express_relay_metadata: u8::MAX,
        }
    }
}
impl<'info> anchor_lang::Bumps for SetRelayer<'info>
where
    'info: 'info,
{
    type Bumps = SetRelayerBumps;
}
/// An internal, Anchor generated module. This is used (as an
/// implementation detail), to generate a struct for a given
/// `#[derive(Accounts)]` implementation, where each field is a Pubkey,
/// instead of an `AccountInfo`. This is useful for clients that want
/// to generate a list of accounts, without explicitly knowing the
/// order all the fields should be in.
///
/// To access the struct in this module, one should use the sibling
/// `accounts` module (also generated), which re-exports this.
pub(crate) mod __client_accounts_set_relayer {
    use super::*;
    use anchor_lang::prelude::borsh;
    /// Generated client accounts for [`SetRelayer`].
    pub struct SetRelayer {
        pub admin: Pubkey,
        pub express_relay_metadata: Pubkey,
        pub relayer_signer: Pubkey,
        pub fee_receiver_relayer: Pubkey,
    }
    impl borsh::ser::BorshSerialize for SetRelayer
    where
        Pubkey: borsh::ser::BorshSerialize,
        Pubkey: borsh::ser::BorshSerialize,
        Pubkey: borsh::ser::BorshSerialize,
        Pubkey: borsh::ser::BorshSerialize,
    {
        fn serialize<W: borsh::maybestd::io::Write>(
            &self,
            writer: &mut W,
        ) -> ::core::result::Result<(), borsh::maybestd::io::Error> {
            borsh::BorshSerialize::serialize(&self.admin, writer)?;
            borsh::BorshSerialize::serialize(&self.express_relay_metadata, writer)?;
            borsh::BorshSerialize::serialize(&self.relayer_signer, writer)?;
            borsh::BorshSerialize::serialize(&self.fee_receiver_relayer, writer)?;
            Ok(())
        }
    }
    #[automatically_derived]
    impl anchor_lang::ToAccountMetas for SetRelayer {
        fn to_account_metas(
            &self,
            is_signer: Option<bool>,
        ) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
            let mut account_metas = ::alloc::vec::Vec::new();
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        self.admin,
                        true,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        self.express_relay_metadata,
                        false,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        self.relayer_signer,
                        false,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        self.fee_receiver_relayer,
                        false,
                    ),
                );
            account_metas
        }
    }
}
/// An internal, Anchor generated module. This is used (as an
/// implementation detail), to generate a CPI struct for a given
/// `#[derive(Accounts)]` implementation, where each field is an
/// AccountInfo.
///
/// To access the struct in this module, one should use the sibling
/// [`cpi::accounts`] module (also generated), which re-exports this.
pub(crate) mod __cpi_client_accounts_set_relayer {
    use super::*;
    /// Generated CPI struct of the accounts for [`SetRelayer`].
    pub struct SetRelayer<'info> {
        pub admin: anchor_lang::solana_program::account_info::AccountInfo<'info>,
        pub express_relay_metadata: anchor_lang::solana_program::account_info::AccountInfo<
            'info,
        >,
        pub relayer_signer: anchor_lang::solana_program::account_info::AccountInfo<
            'info,
        >,
        pub fee_receiver_relayer: anchor_lang::solana_program::account_info::AccountInfo<
            'info,
        >,
    }
    #[automatically_derived]
    impl<'info> anchor_lang::ToAccountMetas for SetRelayer<'info> {
        fn to_account_metas(
            &self,
            is_signer: Option<bool>,
        ) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
            let mut account_metas = ::alloc::vec::Vec::new();
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        anchor_lang::Key::key(&self.admin),
                        true,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        anchor_lang::Key::key(&self.express_relay_metadata),
                        false,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        anchor_lang::Key::key(&self.relayer_signer),
                        false,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        anchor_lang::Key::key(&self.fee_receiver_relayer),
                        false,
                    ),
                );
            account_metas
        }
    }
    #[automatically_derived]
    impl<'info> anchor_lang::ToAccountInfos<'info> for SetRelayer<'info> {
        fn to_account_infos(
            &self,
        ) -> Vec<anchor_lang::solana_program::account_info::AccountInfo<'info>> {
            let mut account_infos = ::alloc::vec::Vec::new();
            account_infos
                .extend(anchor_lang::ToAccountInfos::to_account_infos(&self.admin));
            account_infos
                .extend(
                    anchor_lang::ToAccountInfos::to_account_infos(
                        &self.express_relay_metadata,
                    ),
                );
            account_infos
                .extend(
                    anchor_lang::ToAccountInfos::to_account_infos(&self.relayer_signer),
                );
            account_infos
                .extend(
                    anchor_lang::ToAccountInfos::to_account_infos(
                        &self.fee_receiver_relayer,
                    ),
                );
            account_infos
        }
    }
}
pub struct SetSplitsArgs {
    pub split_router_default: u64,
    pub split_relayer: u64,
}
impl borsh::ser::BorshSerialize for SetSplitsArgs
where
    u64: borsh::ser::BorshSerialize,
    u64: borsh::ser::BorshSerialize,
{
    fn serialize<W: borsh::maybestd::io::Write>(
        &self,
        writer: &mut W,
    ) -> ::core::result::Result<(), borsh::maybestd::io::Error> {
        borsh::BorshSerialize::serialize(&self.split_router_default, writer)?;
        borsh::BorshSerialize::serialize(&self.split_relayer, writer)?;
        Ok(())
    }
}
impl borsh::de::BorshDeserialize for SetSplitsArgs
where
    u64: borsh::BorshDeserialize,
    u64: borsh::BorshDeserialize,
{
    fn deserialize_reader<R: borsh::maybestd::io::Read>(
        reader: &mut R,
    ) -> ::core::result::Result<Self, borsh::maybestd::io::Error> {
        Ok(Self {
            split_router_default: borsh::BorshDeserialize::deserialize_reader(reader)?,
            split_relayer: borsh::BorshDeserialize::deserialize_reader(reader)?,
        })
    }
}
#[automatically_derived]
impl ::core::cmp::Eq for SetSplitsArgs {
    #[inline]
    #[doc(hidden)]
    #[coverage(off)]
    fn assert_receiver_is_total_eq(&self) -> () {
        let _: ::core::cmp::AssertParamIsEq<u64>;
    }
}
#[automatically_derived]
impl ::core::marker::StructuralPartialEq for SetSplitsArgs {}
#[automatically_derived]
impl ::core::cmp::PartialEq for SetSplitsArgs {
    #[inline]
    fn eq(&self, other: &SetSplitsArgs) -> bool {
        self.split_router_default == other.split_router_default
            && self.split_relayer == other.split_relayer
    }
}
#[automatically_derived]
impl ::core::clone::Clone for SetSplitsArgs {
    #[inline]
    fn clone(&self) -> SetSplitsArgs {
        let _: ::core::clone::AssertParamIsClone<u64>;
        *self
    }
}
#[automatically_derived]
impl ::core::marker::Copy for SetSplitsArgs {}
#[automatically_derived]
impl ::core::fmt::Debug for SetSplitsArgs {
    #[inline]
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        ::core::fmt::Formatter::debug_struct_field2_finish(
            f,
            "SetSplitsArgs",
            "split_router_default",
            &self.split_router_default,
            "split_relayer",
            &&self.split_relayer,
        )
    }
}
pub struct SetSplits<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,
    #[account(mut, seeds = [SEED_METADATA], bump, has_one = admin)]
    pub express_relay_metadata: Account<'info, ExpressRelayMetadata>,
}
#[automatically_derived]
impl<'info> anchor_lang::Accounts<'info, SetSplitsBumps> for SetSplits<'info>
where
    'info: 'info,
{
    #[inline(never)]
    fn try_accounts(
        __program_id: &anchor_lang::solana_program::pubkey::Pubkey,
        __accounts: &mut &'info [anchor_lang::solana_program::account_info::AccountInfo<
            'info,
        >],
        __ix_data: &[u8],
        __bumps: &mut SetSplitsBumps,
        __reallocs: &mut std::collections::BTreeSet<
            anchor_lang::solana_program::pubkey::Pubkey,
        >,
    ) -> anchor_lang::Result<Self> {
        let admin: Signer = anchor_lang::Accounts::try_accounts(
                __program_id,
                __accounts,
                __ix_data,
                __bumps,
                __reallocs,
            )
            .map_err(|e| e.with_account_name("admin"))?;
        let express_relay_metadata: anchor_lang::accounts::account::Account<
            ExpressRelayMetadata,
        > = anchor_lang::Accounts::try_accounts(
                __program_id,
                __accounts,
                __ix_data,
                __bumps,
                __reallocs,
            )
            .map_err(|e| e.with_account_name("express_relay_metadata"))?;
        if !AsRef::<AccountInfo>::as_ref(&admin).is_writable {
            return Err(
                anchor_lang::error::Error::from(
                        anchor_lang::error::ErrorCode::ConstraintMut,
                    )
                    .with_account_name("admin"),
            );
        }
        let (__pda_address, __bump) = Pubkey::find_program_address(
            &[SEED_METADATA],
            &__program_id,
        );
        __bumps.express_relay_metadata = __bump;
        if express_relay_metadata.key() != __pda_address {
            return Err(
                anchor_lang::error::Error::from(
                        anchor_lang::error::ErrorCode::ConstraintSeeds,
                    )
                    .with_account_name("express_relay_metadata")
                    .with_pubkeys((express_relay_metadata.key(), __pda_address)),
            );
        }
        if !AsRef::<AccountInfo>::as_ref(&express_relay_metadata).is_writable {
            return Err(
                anchor_lang::error::Error::from(
                        anchor_lang::error::ErrorCode::ConstraintMut,
                    )
                    .with_account_name("express_relay_metadata"),
            );
        }
        {
            let my_key = express_relay_metadata.admin;
            let target_key = admin.key();
            if my_key != target_key {
                return Err(
                    anchor_lang::error::Error::from(
                            anchor_lang::error::ErrorCode::ConstraintHasOne,
                        )
                        .with_account_name("express_relay_metadata")
                        .with_pubkeys((my_key, target_key)),
                );
            }
        }
        Ok(SetSplits {
            admin,
            express_relay_metadata,
        })
    }
}
#[automatically_derived]
impl<'info> anchor_lang::ToAccountInfos<'info> for SetSplits<'info>
where
    'info: 'info,
{
    fn to_account_infos(
        &self,
    ) -> Vec<anchor_lang::solana_program::account_info::AccountInfo<'info>> {
        let mut account_infos = ::alloc::vec::Vec::new();
        account_infos.extend(self.admin.to_account_infos());
        account_infos.extend(self.express_relay_metadata.to_account_infos());
        account_infos
    }
}
#[automatically_derived]
impl<'info> anchor_lang::ToAccountMetas for SetSplits<'info> {
    fn to_account_metas(
        &self,
        is_signer: Option<bool>,
    ) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
        let mut account_metas = ::alloc::vec::Vec::new();
        account_metas.extend(self.admin.to_account_metas(None));
        account_metas.extend(self.express_relay_metadata.to_account_metas(None));
        account_metas
    }
}
#[automatically_derived]
impl<'info> anchor_lang::AccountsExit<'info> for SetSplits<'info>
where
    'info: 'info,
{
    fn exit(
        &self,
        program_id: &anchor_lang::solana_program::pubkey::Pubkey,
    ) -> anchor_lang::Result<()> {
        anchor_lang::AccountsExit::exit(&self.admin, program_id)
            .map_err(|e| e.with_account_name("admin"))?;
        anchor_lang::AccountsExit::exit(&self.express_relay_metadata, program_id)
            .map_err(|e| e.with_account_name("express_relay_metadata"))?;
        Ok(())
    }
}
pub struct SetSplitsBumps {
    pub express_relay_metadata: u8,
}
#[automatically_derived]
impl ::core::fmt::Debug for SetSplitsBumps {
    #[inline]
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        ::core::fmt::Formatter::debug_struct_field1_finish(
            f,
            "SetSplitsBumps",
            "express_relay_metadata",
            &&self.express_relay_metadata,
        )
    }
}
impl Default for SetSplitsBumps {
    fn default() -> Self {
        SetSplitsBumps {
            express_relay_metadata: u8::MAX,
        }
    }
}
impl<'info> anchor_lang::Bumps for SetSplits<'info>
where
    'info: 'info,
{
    type Bumps = SetSplitsBumps;
}
/// An internal, Anchor generated module. This is used (as an
/// implementation detail), to generate a struct for a given
/// `#[derive(Accounts)]` implementation, where each field is a Pubkey,
/// instead of an `AccountInfo`. This is useful for clients that want
/// to generate a list of accounts, without explicitly knowing the
/// order all the fields should be in.
///
/// To access the struct in this module, one should use the sibling
/// `accounts` module (also generated), which re-exports this.
pub(crate) mod __client_accounts_set_splits {
    use super::*;
    use anchor_lang::prelude::borsh;
    /// Generated client accounts for [`SetSplits`].
    pub struct SetSplits {
        pub admin: Pubkey,
        pub express_relay_metadata: Pubkey,
    }
    impl borsh::ser::BorshSerialize for SetSplits
    where
        Pubkey: borsh::ser::BorshSerialize,
        Pubkey: borsh::ser::BorshSerialize,
    {
        fn serialize<W: borsh::maybestd::io::Write>(
            &self,
            writer: &mut W,
        ) -> ::core::result::Result<(), borsh::maybestd::io::Error> {
            borsh::BorshSerialize::serialize(&self.admin, writer)?;
            borsh::BorshSerialize::serialize(&self.express_relay_metadata, writer)?;
            Ok(())
        }
    }
    #[automatically_derived]
    impl anchor_lang::ToAccountMetas for SetSplits {
        fn to_account_metas(
            &self,
            is_signer: Option<bool>,
        ) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
            let mut account_metas = ::alloc::vec::Vec::new();
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        self.admin,
                        true,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        self.express_relay_metadata,
                        false,
                    ),
                );
            account_metas
        }
    }
}
/// An internal, Anchor generated module. This is used (as an
/// implementation detail), to generate a CPI struct for a given
/// `#[derive(Accounts)]` implementation, where each field is an
/// AccountInfo.
///
/// To access the struct in this module, one should use the sibling
/// [`cpi::accounts`] module (also generated), which re-exports this.
pub(crate) mod __cpi_client_accounts_set_splits {
    use super::*;
    /// Generated CPI struct of the accounts for [`SetSplits`].
    pub struct SetSplits<'info> {
        pub admin: anchor_lang::solana_program::account_info::AccountInfo<'info>,
        pub express_relay_metadata: anchor_lang::solana_program::account_info::AccountInfo<
            'info,
        >,
    }
    #[automatically_derived]
    impl<'info> anchor_lang::ToAccountMetas for SetSplits<'info> {
        fn to_account_metas(
            &self,
            is_signer: Option<bool>,
        ) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
            let mut account_metas = ::alloc::vec::Vec::new();
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        anchor_lang::Key::key(&self.admin),
                        true,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        anchor_lang::Key::key(&self.express_relay_metadata),
                        false,
                    ),
                );
            account_metas
        }
    }
    #[automatically_derived]
    impl<'info> anchor_lang::ToAccountInfos<'info> for SetSplits<'info> {
        fn to_account_infos(
            &self,
        ) -> Vec<anchor_lang::solana_program::account_info::AccountInfo<'info>> {
            let mut account_infos = ::alloc::vec::Vec::new();
            account_infos
                .extend(anchor_lang::ToAccountInfos::to_account_infos(&self.admin));
            account_infos
                .extend(
                    anchor_lang::ToAccountInfos::to_account_infos(
                        &self.express_relay_metadata,
                    ),
                );
            account_infos
        }
    }
}
pub struct SetRouterSplitArgs {
    pub split_router: u64,
}
impl borsh::ser::BorshSerialize for SetRouterSplitArgs
where
    u64: borsh::ser::BorshSerialize,
{
    fn serialize<W: borsh::maybestd::io::Write>(
        &self,
        writer: &mut W,
    ) -> ::core::result::Result<(), borsh::maybestd::io::Error> {
        borsh::BorshSerialize::serialize(&self.split_router, writer)?;
        Ok(())
    }
}
impl borsh::de::BorshDeserialize for SetRouterSplitArgs
where
    u64: borsh::BorshDeserialize,
{
    fn deserialize_reader<R: borsh::maybestd::io::Read>(
        reader: &mut R,
    ) -> ::core::result::Result<Self, borsh::maybestd::io::Error> {
        Ok(Self {
            split_router: borsh::BorshDeserialize::deserialize_reader(reader)?,
        })
    }
}
#[automatically_derived]
impl ::core::cmp::Eq for SetRouterSplitArgs {
    #[inline]
    #[doc(hidden)]
    #[coverage(off)]
    fn assert_receiver_is_total_eq(&self) -> () {
        let _: ::core::cmp::AssertParamIsEq<u64>;
    }
}
#[automatically_derived]
impl ::core::marker::StructuralPartialEq for SetRouterSplitArgs {}
#[automatically_derived]
impl ::core::cmp::PartialEq for SetRouterSplitArgs {
    #[inline]
    fn eq(&self, other: &SetRouterSplitArgs) -> bool {
        self.split_router == other.split_router
    }
}
#[automatically_derived]
impl ::core::clone::Clone for SetRouterSplitArgs {
    #[inline]
    fn clone(&self) -> SetRouterSplitArgs {
        let _: ::core::clone::AssertParamIsClone<u64>;
        *self
    }
}
#[automatically_derived]
impl ::core::marker::Copy for SetRouterSplitArgs {}
#[automatically_derived]
impl ::core::fmt::Debug for SetRouterSplitArgs {
    #[inline]
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        ::core::fmt::Formatter::debug_struct_field1_finish(
            f,
            "SetRouterSplitArgs",
            "split_router",
            &&self.split_router,
        )
    }
}
pub struct SetRouterSplit<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,
    #[account(
        init_if_needed,
        payer = admin,
        space = RESERVE_EXPRESS_RELAY_CONFIG_ROUTER,
        seeds = [SEED_CONFIG_ROUTER,
        router.key().as_ref()],
        bump
    )]
    pub config_router: Account<'info, ConfigRouter>,
    #[account(seeds = [SEED_METADATA], bump, has_one = admin)]
    pub express_relay_metadata: Account<'info, ExpressRelayMetadata>,
    /// CHECK: this is just the router fee receiver PK.
    pub router: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}
#[automatically_derived]
impl<'info> anchor_lang::Accounts<'info, SetRouterSplitBumps> for SetRouterSplit<'info>
where
    'info: 'info,
{
    #[inline(never)]
    fn try_accounts(
        __program_id: &anchor_lang::solana_program::pubkey::Pubkey,
        __accounts: &mut &'info [anchor_lang::solana_program::account_info::AccountInfo<
            'info,
        >],
        __ix_data: &[u8],
        __bumps: &mut SetRouterSplitBumps,
        __reallocs: &mut std::collections::BTreeSet<
            anchor_lang::solana_program::pubkey::Pubkey,
        >,
    ) -> anchor_lang::Result<Self> {
        let admin: Signer = anchor_lang::Accounts::try_accounts(
                __program_id,
                __accounts,
                __ix_data,
                __bumps,
                __reallocs,
            )
            .map_err(|e| e.with_account_name("admin"))?;
        if __accounts.is_empty() {
            return Err(anchor_lang::error::ErrorCode::AccountNotEnoughKeys.into());
        }
        let config_router = &__accounts[0];
        *__accounts = &__accounts[1..];
        let express_relay_metadata: anchor_lang::accounts::account::Account<
            ExpressRelayMetadata,
        > = anchor_lang::Accounts::try_accounts(
                __program_id,
                __accounts,
                __ix_data,
                __bumps,
                __reallocs,
            )
            .map_err(|e| e.with_account_name("express_relay_metadata"))?;
        let router: UncheckedAccount = anchor_lang::Accounts::try_accounts(
                __program_id,
                __accounts,
                __ix_data,
                __bumps,
                __reallocs,
            )
            .map_err(|e| e.with_account_name("router"))?;
        let system_program: anchor_lang::accounts::program::Program<System> = anchor_lang::Accounts::try_accounts(
                __program_id,
                __accounts,
                __ix_data,
                __bumps,
                __reallocs,
            )
            .map_err(|e| e.with_account_name("system_program"))?;
        let __anchor_rent = Rent::get()?;
        let (__pda_address, __bump) = Pubkey::find_program_address(
            &[SEED_CONFIG_ROUTER, router.key().as_ref()],
            __program_id,
        );
        __bumps.config_router = __bump;
        if config_router.key() != __pda_address {
            return Err(
                anchor_lang::error::Error::from(
                        anchor_lang::error::ErrorCode::ConstraintSeeds,
                    )
                    .with_account_name("config_router")
                    .with_pubkeys((config_router.key(), __pda_address)),
            );
        }
        let config_router = ({
            #[inline(never)]
            || {
                let actual_field = AsRef::<AccountInfo>::as_ref(&config_router);
                let actual_owner = actual_field.owner;
                let space = RESERVE_EXPRESS_RELAY_CONFIG_ROUTER;
                let pa: anchor_lang::accounts::account::Account<ConfigRouter> = if !true
                    || actual_owner == &anchor_lang::solana_program::system_program::ID
                {
                    let __current_lamports = config_router.lamports();
                    if __current_lamports == 0 {
                        let space = space;
                        let lamports = __anchor_rent.minimum_balance(space);
                        let cpi_accounts = anchor_lang::system_program::CreateAccount {
                            from: admin.to_account_info(),
                            to: config_router.to_account_info(),
                        };
                        let cpi_context = anchor_lang::context::CpiContext::new(
                            system_program.to_account_info(),
                            cpi_accounts,
                        );
                        anchor_lang::system_program::create_account(
                            cpi_context
                                .with_signer(
                                    &[
                                        &[
                                            SEED_CONFIG_ROUTER,
                                            router.key().as_ref(),
                                            &[__bump][..],
                                        ][..],
                                    ],
                                ),
                            lamports,
                            space as u64,
                            __program_id,
                        )?;
                    } else {
                        if admin.key() == config_router.key() {
                            return Err(
                                anchor_lang::error::Error::from(anchor_lang::error::AnchorError {
                                        error_name: anchor_lang::error::ErrorCode::TryingToInitPayerAsProgramAccount
                                            .name(),
                                        error_code_number: anchor_lang::error::ErrorCode::TryingToInitPayerAsProgramAccount
                                            .into(),
                                        error_msg: anchor_lang::error::ErrorCode::TryingToInitPayerAsProgramAccount
                                            .to_string(),
                                        error_origin: Some(
                                            anchor_lang::error::ErrorOrigin::Source(anchor_lang::error::Source {
                                                filename: "programs/express_relay/src/lib.rs",
                                                line: 334u32,
                                            }),
                                        ),
                                        compared_values: None,
                                    })
                                    .with_pubkeys((admin.key(), config_router.key())),
                            );
                        }
                        let required_lamports = __anchor_rent
                            .minimum_balance(space)
                            .max(1)
                            .saturating_sub(__current_lamports);
                        if required_lamports > 0 {
                            let cpi_accounts = anchor_lang::system_program::Transfer {
                                from: admin.to_account_info(),
                                to: config_router.to_account_info(),
                            };
                            let cpi_context = anchor_lang::context::CpiContext::new(
                                system_program.to_account_info(),
                                cpi_accounts,
                            );
                            anchor_lang::system_program::transfer(
                                cpi_context,
                                required_lamports,
                            )?;
                        }
                        let cpi_accounts = anchor_lang::system_program::Allocate {
                            account_to_allocate: config_router.to_account_info(),
                        };
                        let cpi_context = anchor_lang::context::CpiContext::new(
                            system_program.to_account_info(),
                            cpi_accounts,
                        );
                        anchor_lang::system_program::allocate(
                            cpi_context
                                .with_signer(
                                    &[
                                        &[
                                            SEED_CONFIG_ROUTER,
                                            router.key().as_ref(),
                                            &[__bump][..],
                                        ][..],
                                    ],
                                ),
                            space as u64,
                        )?;
                        let cpi_accounts = anchor_lang::system_program::Assign {
                            account_to_assign: config_router.to_account_info(),
                        };
                        let cpi_context = anchor_lang::context::CpiContext::new(
                            system_program.to_account_info(),
                            cpi_accounts,
                        );
                        anchor_lang::system_program::assign(
                            cpi_context
                                .with_signer(
                                    &[
                                        &[
                                            SEED_CONFIG_ROUTER,
                                            router.key().as_ref(),
                                            &[__bump][..],
                                        ][..],
                                    ],
                                ),
                            __program_id,
                        )?;
                    }
                    match anchor_lang::accounts::account::Account::try_from_unchecked(
                        &config_router,
                    ) {
                        Ok(val) => val,
                        Err(e) => return Err(e.with_account_name("config_router")),
                    }
                } else {
                    match anchor_lang::accounts::account::Account::try_from(
                        &config_router,
                    ) {
                        Ok(val) => val,
                        Err(e) => return Err(e.with_account_name("config_router")),
                    }
                };
                if true {
                    if space != actual_field.data_len() {
                        return Err(
                            anchor_lang::error::Error::from(
                                    anchor_lang::error::ErrorCode::ConstraintSpace,
                                )
                                .with_account_name("config_router")
                                .with_values((space, actual_field.data_len())),
                        );
                    }
                    if actual_owner != __program_id {
                        return Err(
                            anchor_lang::error::Error::from(
                                    anchor_lang::error::ErrorCode::ConstraintOwner,
                                )
                                .with_account_name("config_router")
                                .with_pubkeys((*actual_owner, *__program_id)),
                        );
                    }
                    {
                        let required_lamports = __anchor_rent.minimum_balance(space);
                        if pa.to_account_info().lamports() < required_lamports {
                            return Err(
                                anchor_lang::error::Error::from(
                                        anchor_lang::error::ErrorCode::ConstraintRentExempt,
                                    )
                                    .with_account_name("config_router"),
                            );
                        }
                    }
                }
                Ok(pa)
            }
        })()?;
        if !AsRef::<AccountInfo>::as_ref(&config_router).is_writable {
            return Err(
                anchor_lang::error::Error::from(
                        anchor_lang::error::ErrorCode::ConstraintMut,
                    )
                    .with_account_name("config_router"),
            );
        }
        if !__anchor_rent
            .is_exempt(
                config_router.to_account_info().lamports(),
                config_router.to_account_info().try_data_len()?,
            )
        {
            return Err(
                anchor_lang::error::Error::from(
                        anchor_lang::error::ErrorCode::ConstraintRentExempt,
                    )
                    .with_account_name("config_router"),
            );
        }
        if !AsRef::<AccountInfo>::as_ref(&admin).is_writable {
            return Err(
                anchor_lang::error::Error::from(
                        anchor_lang::error::ErrorCode::ConstraintMut,
                    )
                    .with_account_name("admin"),
            );
        }
        let (__pda_address, __bump) = Pubkey::find_program_address(
            &[SEED_METADATA],
            &__program_id,
        );
        __bumps.express_relay_metadata = __bump;
        if express_relay_metadata.key() != __pda_address {
            return Err(
                anchor_lang::error::Error::from(
                        anchor_lang::error::ErrorCode::ConstraintSeeds,
                    )
                    .with_account_name("express_relay_metadata")
                    .with_pubkeys((express_relay_metadata.key(), __pda_address)),
            );
        }
        {
            let my_key = express_relay_metadata.admin;
            let target_key = admin.key();
            if my_key != target_key {
                return Err(
                    anchor_lang::error::Error::from(
                            anchor_lang::error::ErrorCode::ConstraintHasOne,
                        )
                        .with_account_name("express_relay_metadata")
                        .with_pubkeys((my_key, target_key)),
                );
            }
        }
        Ok(SetRouterSplit {
            admin,
            config_router,
            express_relay_metadata,
            router,
            system_program,
        })
    }
}
#[automatically_derived]
impl<'info> anchor_lang::ToAccountInfos<'info> for SetRouterSplit<'info>
where
    'info: 'info,
{
    fn to_account_infos(
        &self,
    ) -> Vec<anchor_lang::solana_program::account_info::AccountInfo<'info>> {
        let mut account_infos = ::alloc::vec::Vec::new();
        account_infos.extend(self.admin.to_account_infos());
        account_infos.extend(self.config_router.to_account_infos());
        account_infos.extend(self.express_relay_metadata.to_account_infos());
        account_infos.extend(self.router.to_account_infos());
        account_infos.extend(self.system_program.to_account_infos());
        account_infos
    }
}
#[automatically_derived]
impl<'info> anchor_lang::ToAccountMetas for SetRouterSplit<'info> {
    fn to_account_metas(
        &self,
        is_signer: Option<bool>,
    ) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
        let mut account_metas = ::alloc::vec::Vec::new();
        account_metas.extend(self.admin.to_account_metas(None));
        account_metas.extend(self.config_router.to_account_metas(None));
        account_metas.extend(self.express_relay_metadata.to_account_metas(None));
        account_metas.extend(self.router.to_account_metas(None));
        account_metas.extend(self.system_program.to_account_metas(None));
        account_metas
    }
}
#[automatically_derived]
impl<'info> anchor_lang::AccountsExit<'info> for SetRouterSplit<'info>
where
    'info: 'info,
{
    fn exit(
        &self,
        program_id: &anchor_lang::solana_program::pubkey::Pubkey,
    ) -> anchor_lang::Result<()> {
        anchor_lang::AccountsExit::exit(&self.admin, program_id)
            .map_err(|e| e.with_account_name("admin"))?;
        anchor_lang::AccountsExit::exit(&self.config_router, program_id)
            .map_err(|e| e.with_account_name("config_router"))?;
        Ok(())
    }
}
pub struct SetRouterSplitBumps {
    pub config_router: u8,
    pub express_relay_metadata: u8,
}
#[automatically_derived]
impl ::core::fmt::Debug for SetRouterSplitBumps {
    #[inline]
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        ::core::fmt::Formatter::debug_struct_field2_finish(
            f,
            "SetRouterSplitBumps",
            "config_router",
            &self.config_router,
            "express_relay_metadata",
            &&self.express_relay_metadata,
        )
    }
}
impl Default for SetRouterSplitBumps {
    fn default() -> Self {
        SetRouterSplitBumps {
            config_router: u8::MAX,
            express_relay_metadata: u8::MAX,
        }
    }
}
impl<'info> anchor_lang::Bumps for SetRouterSplit<'info>
where
    'info: 'info,
{
    type Bumps = SetRouterSplitBumps;
}
/// An internal, Anchor generated module. This is used (as an
/// implementation detail), to generate a struct for a given
/// `#[derive(Accounts)]` implementation, where each field is a Pubkey,
/// instead of an `AccountInfo`. This is useful for clients that want
/// to generate a list of accounts, without explicitly knowing the
/// order all the fields should be in.
///
/// To access the struct in this module, one should use the sibling
/// `accounts` module (also generated), which re-exports this.
pub(crate) mod __client_accounts_set_router_split {
    use super::*;
    use anchor_lang::prelude::borsh;
    /// Generated client accounts for [`SetRouterSplit`].
    pub struct SetRouterSplit {
        pub admin: Pubkey,
        pub config_router: Pubkey,
        pub express_relay_metadata: Pubkey,
        pub router: Pubkey,
        pub system_program: Pubkey,
    }
    impl borsh::ser::BorshSerialize for SetRouterSplit
    where
        Pubkey: borsh::ser::BorshSerialize,
        Pubkey: borsh::ser::BorshSerialize,
        Pubkey: borsh::ser::BorshSerialize,
        Pubkey: borsh::ser::BorshSerialize,
        Pubkey: borsh::ser::BorshSerialize,
    {
        fn serialize<W: borsh::maybestd::io::Write>(
            &self,
            writer: &mut W,
        ) -> ::core::result::Result<(), borsh::maybestd::io::Error> {
            borsh::BorshSerialize::serialize(&self.admin, writer)?;
            borsh::BorshSerialize::serialize(&self.config_router, writer)?;
            borsh::BorshSerialize::serialize(&self.express_relay_metadata, writer)?;
            borsh::BorshSerialize::serialize(&self.router, writer)?;
            borsh::BorshSerialize::serialize(&self.system_program, writer)?;
            Ok(())
        }
    }
    #[automatically_derived]
    impl anchor_lang::ToAccountMetas for SetRouterSplit {
        fn to_account_metas(
            &self,
            is_signer: Option<bool>,
        ) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
            let mut account_metas = ::alloc::vec::Vec::new();
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        self.admin,
                        true,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        self.config_router,
                        false,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        self.express_relay_metadata,
                        false,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        self.router,
                        false,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        self.system_program,
                        false,
                    ),
                );
            account_metas
        }
    }
}
/// An internal, Anchor generated module. This is used (as an
/// implementation detail), to generate a CPI struct for a given
/// `#[derive(Accounts)]` implementation, where each field is an
/// AccountInfo.
///
/// To access the struct in this module, one should use the sibling
/// [`cpi::accounts`] module (also generated), which re-exports this.
pub(crate) mod __cpi_client_accounts_set_router_split {
    use super::*;
    /// Generated CPI struct of the accounts for [`SetRouterSplit`].
    pub struct SetRouterSplit<'info> {
        pub admin: anchor_lang::solana_program::account_info::AccountInfo<'info>,
        pub config_router: anchor_lang::solana_program::account_info::AccountInfo<'info>,
        pub express_relay_metadata: anchor_lang::solana_program::account_info::AccountInfo<
            'info,
        >,
        pub router: anchor_lang::solana_program::account_info::AccountInfo<'info>,
        pub system_program: anchor_lang::solana_program::account_info::AccountInfo<
            'info,
        >,
    }
    #[automatically_derived]
    impl<'info> anchor_lang::ToAccountMetas for SetRouterSplit<'info> {
        fn to_account_metas(
            &self,
            is_signer: Option<bool>,
        ) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
            let mut account_metas = ::alloc::vec::Vec::new();
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        anchor_lang::Key::key(&self.admin),
                        true,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        anchor_lang::Key::key(&self.config_router),
                        false,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        anchor_lang::Key::key(&self.express_relay_metadata),
                        false,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        anchor_lang::Key::key(&self.router),
                        false,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        anchor_lang::Key::key(&self.system_program),
                        false,
                    ),
                );
            account_metas
        }
    }
    #[automatically_derived]
    impl<'info> anchor_lang::ToAccountInfos<'info> for SetRouterSplit<'info> {
        fn to_account_infos(
            &self,
        ) -> Vec<anchor_lang::solana_program::account_info::AccountInfo<'info>> {
            let mut account_infos = ::alloc::vec::Vec::new();
            account_infos
                .extend(anchor_lang::ToAccountInfos::to_account_infos(&self.admin));
            account_infos
                .extend(
                    anchor_lang::ToAccountInfos::to_account_infos(&self.config_router),
                );
            account_infos
                .extend(
                    anchor_lang::ToAccountInfos::to_account_infos(
                        &self.express_relay_metadata,
                    ),
                );
            account_infos
                .extend(anchor_lang::ToAccountInfos::to_account_infos(&self.router));
            account_infos
                .extend(
                    anchor_lang::ToAccountInfos::to_account_infos(&self.system_program),
                );
            account_infos
        }
    }
}
pub struct SubmitBidArgs {
    pub deadline: i64,
    pub bid_amount: u64,
}
impl borsh::ser::BorshSerialize for SubmitBidArgs
where
    i64: borsh::ser::BorshSerialize,
    u64: borsh::ser::BorshSerialize,
{
    fn serialize<W: borsh::maybestd::io::Write>(
        &self,
        writer: &mut W,
    ) -> ::core::result::Result<(), borsh::maybestd::io::Error> {
        borsh::BorshSerialize::serialize(&self.deadline, writer)?;
        borsh::BorshSerialize::serialize(&self.bid_amount, writer)?;
        Ok(())
    }
}
impl borsh::de::BorshDeserialize for SubmitBidArgs
where
    i64: borsh::BorshDeserialize,
    u64: borsh::BorshDeserialize,
{
    fn deserialize_reader<R: borsh::maybestd::io::Read>(
        reader: &mut R,
    ) -> ::core::result::Result<Self, borsh::maybestd::io::Error> {
        Ok(Self {
            deadline: borsh::BorshDeserialize::deserialize_reader(reader)?,
            bid_amount: borsh::BorshDeserialize::deserialize_reader(reader)?,
        })
    }
}
#[automatically_derived]
impl ::core::cmp::Eq for SubmitBidArgs {
    #[inline]
    #[doc(hidden)]
    #[coverage(off)]
    fn assert_receiver_is_total_eq(&self) -> () {
        let _: ::core::cmp::AssertParamIsEq<i64>;
        let _: ::core::cmp::AssertParamIsEq<u64>;
    }
}
#[automatically_derived]
impl ::core::marker::StructuralPartialEq for SubmitBidArgs {}
#[automatically_derived]
impl ::core::cmp::PartialEq for SubmitBidArgs {
    #[inline]
    fn eq(&self, other: &SubmitBidArgs) -> bool {
        self.deadline == other.deadline && self.bid_amount == other.bid_amount
    }
}
#[automatically_derived]
impl ::core::clone::Clone for SubmitBidArgs {
    #[inline]
    fn clone(&self) -> SubmitBidArgs {
        let _: ::core::clone::AssertParamIsClone<i64>;
        let _: ::core::clone::AssertParamIsClone<u64>;
        *self
    }
}
#[automatically_derived]
impl ::core::marker::Copy for SubmitBidArgs {}
#[automatically_derived]
impl ::core::fmt::Debug for SubmitBidArgs {
    #[inline]
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        ::core::fmt::Formatter::debug_struct_field2_finish(
            f,
            "SubmitBidArgs",
            "deadline",
            &self.deadline,
            "bid_amount",
            &&self.bid_amount,
        )
    }
}
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
    #[account(
        mut,
        seeds = [SEED_METADATA],
        bump,
        has_one = relayer_signer,
        has_one = fee_receiver_relayer
    )]
    pub express_relay_metadata: Account<'info, ExpressRelayMetadata>,
    /// CHECK: this is just a PK for the relayer to receive fees at.
    #[account(mut)]
    pub fee_receiver_relayer: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
    /// CHECK: this is the sysvar instructions account.
    #[account(address = sysvar_instructions::ID)]
    pub sysvar_instructions: UncheckedAccount<'info>,
}
#[automatically_derived]
impl<'info> anchor_lang::Accounts<'info, SubmitBidBumps> for SubmitBid<'info>
where
    'info: 'info,
{
    #[inline(never)]
    fn try_accounts(
        __program_id: &anchor_lang::solana_program::pubkey::Pubkey,
        __accounts: &mut &'info [anchor_lang::solana_program::account_info::AccountInfo<
            'info,
        >],
        __ix_data: &[u8],
        __bumps: &mut SubmitBidBumps,
        __reallocs: &mut std::collections::BTreeSet<
            anchor_lang::solana_program::pubkey::Pubkey,
        >,
    ) -> anchor_lang::Result<Self> {
        let searcher: Signer = anchor_lang::Accounts::try_accounts(
                __program_id,
                __accounts,
                __ix_data,
                __bumps,
                __reallocs,
            )
            .map_err(|e| e.with_account_name("searcher"))?;
        let relayer_signer: Signer = anchor_lang::Accounts::try_accounts(
                __program_id,
                __accounts,
                __ix_data,
                __bumps,
                __reallocs,
            )
            .map_err(|e| e.with_account_name("relayer_signer"))?;
        let permission: UncheckedAccount = anchor_lang::Accounts::try_accounts(
                __program_id,
                __accounts,
                __ix_data,
                __bumps,
                __reallocs,
            )
            .map_err(|e| e.with_account_name("permission"))?;
        let router: UncheckedAccount = anchor_lang::Accounts::try_accounts(
                __program_id,
                __accounts,
                __ix_data,
                __bumps,
                __reallocs,
            )
            .map_err(|e| e.with_account_name("router"))?;
        let config_router: UncheckedAccount = anchor_lang::Accounts::try_accounts(
                __program_id,
                __accounts,
                __ix_data,
                __bumps,
                __reallocs,
            )
            .map_err(|e| e.with_account_name("config_router"))?;
        let express_relay_metadata: anchor_lang::accounts::account::Account<
            ExpressRelayMetadata,
        > = anchor_lang::Accounts::try_accounts(
                __program_id,
                __accounts,
                __ix_data,
                __bumps,
                __reallocs,
            )
            .map_err(|e| e.with_account_name("express_relay_metadata"))?;
        let fee_receiver_relayer: UncheckedAccount = anchor_lang::Accounts::try_accounts(
                __program_id,
                __accounts,
                __ix_data,
                __bumps,
                __reallocs,
            )
            .map_err(|e| e.with_account_name("fee_receiver_relayer"))?;
        let system_program: anchor_lang::accounts::program::Program<System> = anchor_lang::Accounts::try_accounts(
                __program_id,
                __accounts,
                __ix_data,
                __bumps,
                __reallocs,
            )
            .map_err(|e| e.with_account_name("system_program"))?;
        let sysvar_instructions: UncheckedAccount = anchor_lang::Accounts::try_accounts(
                __program_id,
                __accounts,
                __ix_data,
                __bumps,
                __reallocs,
            )
            .map_err(|e| e.with_account_name("sysvar_instructions"))?;
        if !AsRef::<AccountInfo>::as_ref(&searcher).is_writable {
            return Err(
                anchor_lang::error::Error::from(
                        anchor_lang::error::ErrorCode::ConstraintMut,
                    )
                    .with_account_name("searcher"),
            );
        }
        if !AsRef::<AccountInfo>::as_ref(&router).is_writable {
            return Err(
                anchor_lang::error::Error::from(
                        anchor_lang::error::ErrorCode::ConstraintMut,
                    )
                    .with_account_name("router"),
            );
        }
        let (__pda_address, __bump) = Pubkey::find_program_address(
            &[SEED_CONFIG_ROUTER, router.key().as_ref()],
            &__program_id,
        );
        __bumps.config_router = __bump;
        if config_router.key() != __pda_address {
            return Err(
                anchor_lang::error::Error::from(
                        anchor_lang::error::ErrorCode::ConstraintSeeds,
                    )
                    .with_account_name("config_router")
                    .with_pubkeys((config_router.key(), __pda_address)),
            );
        }
        let (__pda_address, __bump) = Pubkey::find_program_address(
            &[SEED_METADATA],
            &__program_id,
        );
        __bumps.express_relay_metadata = __bump;
        if express_relay_metadata.key() != __pda_address {
            return Err(
                anchor_lang::error::Error::from(
                        anchor_lang::error::ErrorCode::ConstraintSeeds,
                    )
                    .with_account_name("express_relay_metadata")
                    .with_pubkeys((express_relay_metadata.key(), __pda_address)),
            );
        }
        if !AsRef::<AccountInfo>::as_ref(&express_relay_metadata).is_writable {
            return Err(
                anchor_lang::error::Error::from(
                        anchor_lang::error::ErrorCode::ConstraintMut,
                    )
                    .with_account_name("express_relay_metadata"),
            );
        }
        {
            let my_key = express_relay_metadata.relayer_signer;
            let target_key = relayer_signer.key();
            if my_key != target_key {
                return Err(
                    anchor_lang::error::Error::from(
                            anchor_lang::error::ErrorCode::ConstraintHasOne,
                        )
                        .with_account_name("express_relay_metadata")
                        .with_pubkeys((my_key, target_key)),
                );
            }
        }
        {
            let my_key = express_relay_metadata.fee_receiver_relayer;
            let target_key = fee_receiver_relayer.key();
            if my_key != target_key {
                return Err(
                    anchor_lang::error::Error::from(
                            anchor_lang::error::ErrorCode::ConstraintHasOne,
                        )
                        .with_account_name("express_relay_metadata")
                        .with_pubkeys((my_key, target_key)),
                );
            }
        }
        if !AsRef::<AccountInfo>::as_ref(&fee_receiver_relayer).is_writable {
            return Err(
                anchor_lang::error::Error::from(
                        anchor_lang::error::ErrorCode::ConstraintMut,
                    )
                    .with_account_name("fee_receiver_relayer"),
            );
        }
        {
            let actual = sysvar_instructions.key();
            let expected = sysvar_instructions::ID;
            if actual != expected {
                return Err(
                    anchor_lang::error::Error::from(
                            anchor_lang::error::ErrorCode::ConstraintAddress,
                        )
                        .with_account_name("sysvar_instructions")
                        .with_pubkeys((actual, expected)),
                );
            }
        }
        Ok(SubmitBid {
            searcher,
            relayer_signer,
            permission,
            router,
            config_router,
            express_relay_metadata,
            fee_receiver_relayer,
            system_program,
            sysvar_instructions,
        })
    }
}
#[automatically_derived]
impl<'info> anchor_lang::ToAccountInfos<'info> for SubmitBid<'info>
where
    'info: 'info,
{
    fn to_account_infos(
        &self,
    ) -> Vec<anchor_lang::solana_program::account_info::AccountInfo<'info>> {
        let mut account_infos = ::alloc::vec::Vec::new();
        account_infos.extend(self.searcher.to_account_infos());
        account_infos.extend(self.relayer_signer.to_account_infos());
        account_infos.extend(self.permission.to_account_infos());
        account_infos.extend(self.router.to_account_infos());
        account_infos.extend(self.config_router.to_account_infos());
        account_infos.extend(self.express_relay_metadata.to_account_infos());
        account_infos.extend(self.fee_receiver_relayer.to_account_infos());
        account_infos.extend(self.system_program.to_account_infos());
        account_infos.extend(self.sysvar_instructions.to_account_infos());
        account_infos
    }
}
#[automatically_derived]
impl<'info> anchor_lang::ToAccountMetas for SubmitBid<'info> {
    fn to_account_metas(
        &self,
        is_signer: Option<bool>,
    ) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
        let mut account_metas = ::alloc::vec::Vec::new();
        account_metas.extend(self.searcher.to_account_metas(None));
        account_metas.extend(self.relayer_signer.to_account_metas(None));
        account_metas.extend(self.permission.to_account_metas(None));
        account_metas.extend(self.router.to_account_metas(None));
        account_metas.extend(self.config_router.to_account_metas(None));
        account_metas.extend(self.express_relay_metadata.to_account_metas(None));
        account_metas.extend(self.fee_receiver_relayer.to_account_metas(None));
        account_metas.extend(self.system_program.to_account_metas(None));
        account_metas.extend(self.sysvar_instructions.to_account_metas(None));
        account_metas
    }
}
#[automatically_derived]
impl<'info> anchor_lang::AccountsExit<'info> for SubmitBid<'info>
where
    'info: 'info,
{
    fn exit(
        &self,
        program_id: &anchor_lang::solana_program::pubkey::Pubkey,
    ) -> anchor_lang::Result<()> {
        anchor_lang::AccountsExit::exit(&self.searcher, program_id)
            .map_err(|e| e.with_account_name("searcher"))?;
        anchor_lang::AccountsExit::exit(&self.router, program_id)
            .map_err(|e| e.with_account_name("router"))?;
        anchor_lang::AccountsExit::exit(&self.express_relay_metadata, program_id)
            .map_err(|e| e.with_account_name("express_relay_metadata"))?;
        anchor_lang::AccountsExit::exit(&self.fee_receiver_relayer, program_id)
            .map_err(|e| e.with_account_name("fee_receiver_relayer"))?;
        Ok(())
    }
}
pub struct SubmitBidBumps {
    pub config_router: u8,
    pub express_relay_metadata: u8,
}
#[automatically_derived]
impl ::core::fmt::Debug for SubmitBidBumps {
    #[inline]
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        ::core::fmt::Formatter::debug_struct_field2_finish(
            f,
            "SubmitBidBumps",
            "config_router",
            &self.config_router,
            "express_relay_metadata",
            &&self.express_relay_metadata,
        )
    }
}
impl Default for SubmitBidBumps {
    fn default() -> Self {
        SubmitBidBumps {
            config_router: u8::MAX,
            express_relay_metadata: u8::MAX,
        }
    }
}
impl<'info> anchor_lang::Bumps for SubmitBid<'info>
where
    'info: 'info,
{
    type Bumps = SubmitBidBumps;
}
/// An internal, Anchor generated module. This is used (as an
/// implementation detail), to generate a struct for a given
/// `#[derive(Accounts)]` implementation, where each field is a Pubkey,
/// instead of an `AccountInfo`. This is useful for clients that want
/// to generate a list of accounts, without explicitly knowing the
/// order all the fields should be in.
///
/// To access the struct in this module, one should use the sibling
/// `accounts` module (also generated), which re-exports this.
pub(crate) mod __client_accounts_submit_bid {
    use super::*;
    use anchor_lang::prelude::borsh;
    /// Generated client accounts for [`SubmitBid`].
    pub struct SubmitBid {
        pub searcher: Pubkey,
        pub relayer_signer: Pubkey,
        pub permission: Pubkey,
        pub router: Pubkey,
        pub config_router: Pubkey,
        pub express_relay_metadata: Pubkey,
        pub fee_receiver_relayer: Pubkey,
        pub system_program: Pubkey,
        pub sysvar_instructions: Pubkey,
    }
    impl borsh::ser::BorshSerialize for SubmitBid
    where
        Pubkey: borsh::ser::BorshSerialize,
        Pubkey: borsh::ser::BorshSerialize,
        Pubkey: borsh::ser::BorshSerialize,
        Pubkey: borsh::ser::BorshSerialize,
        Pubkey: borsh::ser::BorshSerialize,
        Pubkey: borsh::ser::BorshSerialize,
        Pubkey: borsh::ser::BorshSerialize,
        Pubkey: borsh::ser::BorshSerialize,
        Pubkey: borsh::ser::BorshSerialize,
    {
        fn serialize<W: borsh::maybestd::io::Write>(
            &self,
            writer: &mut W,
        ) -> ::core::result::Result<(), borsh::maybestd::io::Error> {
            borsh::BorshSerialize::serialize(&self.searcher, writer)?;
            borsh::BorshSerialize::serialize(&self.relayer_signer, writer)?;
            borsh::BorshSerialize::serialize(&self.permission, writer)?;
            borsh::BorshSerialize::serialize(&self.router, writer)?;
            borsh::BorshSerialize::serialize(&self.config_router, writer)?;
            borsh::BorshSerialize::serialize(&self.express_relay_metadata, writer)?;
            borsh::BorshSerialize::serialize(&self.fee_receiver_relayer, writer)?;
            borsh::BorshSerialize::serialize(&self.system_program, writer)?;
            borsh::BorshSerialize::serialize(&self.sysvar_instructions, writer)?;
            Ok(())
        }
    }
    #[automatically_derived]
    impl anchor_lang::ToAccountMetas for SubmitBid {
        fn to_account_metas(
            &self,
            is_signer: Option<bool>,
        ) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
            let mut account_metas = ::alloc::vec::Vec::new();
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        self.searcher,
                        true,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        self.relayer_signer,
                        true,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        self.permission,
                        false,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        self.router,
                        false,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        self.config_router,
                        false,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        self.express_relay_metadata,
                        false,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        self.fee_receiver_relayer,
                        false,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        self.system_program,
                        false,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        self.sysvar_instructions,
                        false,
                    ),
                );
            account_metas
        }
    }
}
/// An internal, Anchor generated module. This is used (as an
/// implementation detail), to generate a CPI struct for a given
/// `#[derive(Accounts)]` implementation, where each field is an
/// AccountInfo.
///
/// To access the struct in this module, one should use the sibling
/// [`cpi::accounts`] module (also generated), which re-exports this.
pub(crate) mod __cpi_client_accounts_submit_bid {
    use super::*;
    /// Generated CPI struct of the accounts for [`SubmitBid`].
    pub struct SubmitBid<'info> {
        pub searcher: anchor_lang::solana_program::account_info::AccountInfo<'info>,
        pub relayer_signer: anchor_lang::solana_program::account_info::AccountInfo<
            'info,
        >,
        pub permission: anchor_lang::solana_program::account_info::AccountInfo<'info>,
        pub router: anchor_lang::solana_program::account_info::AccountInfo<'info>,
        pub config_router: anchor_lang::solana_program::account_info::AccountInfo<'info>,
        pub express_relay_metadata: anchor_lang::solana_program::account_info::AccountInfo<
            'info,
        >,
        pub fee_receiver_relayer: anchor_lang::solana_program::account_info::AccountInfo<
            'info,
        >,
        pub system_program: anchor_lang::solana_program::account_info::AccountInfo<
            'info,
        >,
        pub sysvar_instructions: anchor_lang::solana_program::account_info::AccountInfo<
            'info,
        >,
    }
    #[automatically_derived]
    impl<'info> anchor_lang::ToAccountMetas for SubmitBid<'info> {
        fn to_account_metas(
            &self,
            is_signer: Option<bool>,
        ) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
            let mut account_metas = ::alloc::vec::Vec::new();
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        anchor_lang::Key::key(&self.searcher),
                        true,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        anchor_lang::Key::key(&self.relayer_signer),
                        true,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        anchor_lang::Key::key(&self.permission),
                        false,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        anchor_lang::Key::key(&self.router),
                        false,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        anchor_lang::Key::key(&self.config_router),
                        false,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        anchor_lang::Key::key(&self.express_relay_metadata),
                        false,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        anchor_lang::Key::key(&self.fee_receiver_relayer),
                        false,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        anchor_lang::Key::key(&self.system_program),
                        false,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        anchor_lang::Key::key(&self.sysvar_instructions),
                        false,
                    ),
                );
            account_metas
        }
    }
    #[automatically_derived]
    impl<'info> anchor_lang::ToAccountInfos<'info> for SubmitBid<'info> {
        fn to_account_infos(
            &self,
        ) -> Vec<anchor_lang::solana_program::account_info::AccountInfo<'info>> {
            let mut account_infos = ::alloc::vec::Vec::new();
            account_infos
                .extend(anchor_lang::ToAccountInfos::to_account_infos(&self.searcher));
            account_infos
                .extend(
                    anchor_lang::ToAccountInfos::to_account_infos(&self.relayer_signer),
                );
            account_infos
                .extend(anchor_lang::ToAccountInfos::to_account_infos(&self.permission));
            account_infos
                .extend(anchor_lang::ToAccountInfos::to_account_infos(&self.router));
            account_infos
                .extend(
                    anchor_lang::ToAccountInfos::to_account_infos(&self.config_router),
                );
            account_infos
                .extend(
                    anchor_lang::ToAccountInfos::to_account_infos(
                        &self.express_relay_metadata,
                    ),
                );
            account_infos
                .extend(
                    anchor_lang::ToAccountInfos::to_account_infos(
                        &self.fee_receiver_relayer,
                    ),
                );
            account_infos
                .extend(
                    anchor_lang::ToAccountInfos::to_account_infos(&self.system_program),
                );
            account_infos
                .extend(
                    anchor_lang::ToAccountInfos::to_account_infos(
                        &self.sysvar_instructions,
                    ),
                );
            account_infos
        }
    }
}
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
#[automatically_derived]
impl<'info> anchor_lang::Accounts<'info, CheckPermissionBumps> for CheckPermission<'info>
where
    'info: 'info,
{
    #[inline(never)]
    fn try_accounts(
        __program_id: &anchor_lang::solana_program::pubkey::Pubkey,
        __accounts: &mut &'info [anchor_lang::solana_program::account_info::AccountInfo<
            'info,
        >],
        __ix_data: &[u8],
        __bumps: &mut CheckPermissionBumps,
        __reallocs: &mut std::collections::BTreeSet<
            anchor_lang::solana_program::pubkey::Pubkey,
        >,
    ) -> anchor_lang::Result<Self> {
        let sysvar_instructions: UncheckedAccount = anchor_lang::Accounts::try_accounts(
                __program_id,
                __accounts,
                __ix_data,
                __bumps,
                __reallocs,
            )
            .map_err(|e| e.with_account_name("sysvar_instructions"))?;
        let permission: UncheckedAccount = anchor_lang::Accounts::try_accounts(
                __program_id,
                __accounts,
                __ix_data,
                __bumps,
                __reallocs,
            )
            .map_err(|e| e.with_account_name("permission"))?;
        let router: UncheckedAccount = anchor_lang::Accounts::try_accounts(
                __program_id,
                __accounts,
                __ix_data,
                __bumps,
                __reallocs,
            )
            .map_err(|e| e.with_account_name("router"))?;
        let config_router: UncheckedAccount = anchor_lang::Accounts::try_accounts(
                __program_id,
                __accounts,
                __ix_data,
                __bumps,
                __reallocs,
            )
            .map_err(|e| e.with_account_name("config_router"))?;
        let express_relay_metadata: anchor_lang::accounts::account::Account<
            ExpressRelayMetadata,
        > = anchor_lang::Accounts::try_accounts(
                __program_id,
                __accounts,
                __ix_data,
                __bumps,
                __reallocs,
            )
            .map_err(|e| e.with_account_name("express_relay_metadata"))?;
        {
            let actual = sysvar_instructions.key();
            let expected = sysvar_instructions::ID;
            if actual != expected {
                return Err(
                    anchor_lang::error::Error::from(
                            anchor_lang::error::ErrorCode::ConstraintAddress,
                        )
                        .with_account_name("sysvar_instructions")
                        .with_pubkeys((actual, expected)),
                );
            }
        }
        let (__pda_address, __bump) = Pubkey::find_program_address(
            &[SEED_CONFIG_ROUTER, router.key().as_ref()],
            &__program_id,
        );
        __bumps.config_router = __bump;
        if config_router.key() != __pda_address {
            return Err(
                anchor_lang::error::Error::from(
                        anchor_lang::error::ErrorCode::ConstraintSeeds,
                    )
                    .with_account_name("config_router")
                    .with_pubkeys((config_router.key(), __pda_address)),
            );
        }
        let (__pda_address, __bump) = Pubkey::find_program_address(
            &[SEED_METADATA],
            &__program_id,
        );
        __bumps.express_relay_metadata = __bump;
        if express_relay_metadata.key() != __pda_address {
            return Err(
                anchor_lang::error::Error::from(
                        anchor_lang::error::ErrorCode::ConstraintSeeds,
                    )
                    .with_account_name("express_relay_metadata")
                    .with_pubkeys((express_relay_metadata.key(), __pda_address)),
            );
        }
        Ok(CheckPermission {
            sysvar_instructions,
            permission,
            router,
            config_router,
            express_relay_metadata,
        })
    }
}
#[automatically_derived]
impl<'info> anchor_lang::ToAccountInfos<'info> for CheckPermission<'info>
where
    'info: 'info,
{
    fn to_account_infos(
        &self,
    ) -> Vec<anchor_lang::solana_program::account_info::AccountInfo<'info>> {
        let mut account_infos = ::alloc::vec::Vec::new();
        account_infos.extend(self.sysvar_instructions.to_account_infos());
        account_infos.extend(self.permission.to_account_infos());
        account_infos.extend(self.router.to_account_infos());
        account_infos.extend(self.config_router.to_account_infos());
        account_infos.extend(self.express_relay_metadata.to_account_infos());
        account_infos
    }
}
#[automatically_derived]
impl<'info> anchor_lang::ToAccountMetas for CheckPermission<'info> {
    fn to_account_metas(
        &self,
        is_signer: Option<bool>,
    ) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
        let mut account_metas = ::alloc::vec::Vec::new();
        account_metas.extend(self.sysvar_instructions.to_account_metas(None));
        account_metas.extend(self.permission.to_account_metas(None));
        account_metas.extend(self.router.to_account_metas(None));
        account_metas.extend(self.config_router.to_account_metas(None));
        account_metas.extend(self.express_relay_metadata.to_account_metas(None));
        account_metas
    }
}
#[automatically_derived]
impl<'info> anchor_lang::AccountsExit<'info> for CheckPermission<'info>
where
    'info: 'info,
{
    fn exit(
        &self,
        program_id: &anchor_lang::solana_program::pubkey::Pubkey,
    ) -> anchor_lang::Result<()> {
        Ok(())
    }
}
pub struct CheckPermissionBumps {
    pub config_router: u8,
    pub express_relay_metadata: u8,
}
#[automatically_derived]
impl ::core::fmt::Debug for CheckPermissionBumps {
    #[inline]
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        ::core::fmt::Formatter::debug_struct_field2_finish(
            f,
            "CheckPermissionBumps",
            "config_router",
            &self.config_router,
            "express_relay_metadata",
            &&self.express_relay_metadata,
        )
    }
}
impl Default for CheckPermissionBumps {
    fn default() -> Self {
        CheckPermissionBumps {
            config_router: u8::MAX,
            express_relay_metadata: u8::MAX,
        }
    }
}
impl<'info> anchor_lang::Bumps for CheckPermission<'info>
where
    'info: 'info,
{
    type Bumps = CheckPermissionBumps;
}
/// An internal, Anchor generated module. This is used (as an
/// implementation detail), to generate a struct for a given
/// `#[derive(Accounts)]` implementation, where each field is a Pubkey,
/// instead of an `AccountInfo`. This is useful for clients that want
/// to generate a list of accounts, without explicitly knowing the
/// order all the fields should be in.
///
/// To access the struct in this module, one should use the sibling
/// `accounts` module (also generated), which re-exports this.
pub(crate) mod __client_accounts_check_permission {
    use super::*;
    use anchor_lang::prelude::borsh;
    /// Generated client accounts for [`CheckPermission`].
    pub struct CheckPermission {
        pub sysvar_instructions: Pubkey,
        pub permission: Pubkey,
        pub router: Pubkey,
        pub config_router: Pubkey,
        pub express_relay_metadata: Pubkey,
    }
    impl borsh::ser::BorshSerialize for CheckPermission
    where
        Pubkey: borsh::ser::BorshSerialize,
        Pubkey: borsh::ser::BorshSerialize,
        Pubkey: borsh::ser::BorshSerialize,
        Pubkey: borsh::ser::BorshSerialize,
        Pubkey: borsh::ser::BorshSerialize,
    {
        fn serialize<W: borsh::maybestd::io::Write>(
            &self,
            writer: &mut W,
        ) -> ::core::result::Result<(), borsh::maybestd::io::Error> {
            borsh::BorshSerialize::serialize(&self.sysvar_instructions, writer)?;
            borsh::BorshSerialize::serialize(&self.permission, writer)?;
            borsh::BorshSerialize::serialize(&self.router, writer)?;
            borsh::BorshSerialize::serialize(&self.config_router, writer)?;
            borsh::BorshSerialize::serialize(&self.express_relay_metadata, writer)?;
            Ok(())
        }
    }
    #[automatically_derived]
    impl anchor_lang::ToAccountMetas for CheckPermission {
        fn to_account_metas(
            &self,
            is_signer: Option<bool>,
        ) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
            let mut account_metas = ::alloc::vec::Vec::new();
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        self.sysvar_instructions,
                        false,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        self.permission,
                        false,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        self.router,
                        false,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        self.config_router,
                        false,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        self.express_relay_metadata,
                        false,
                    ),
                );
            account_metas
        }
    }
}
/// An internal, Anchor generated module. This is used (as an
/// implementation detail), to generate a CPI struct for a given
/// `#[derive(Accounts)]` implementation, where each field is an
/// AccountInfo.
///
/// To access the struct in this module, one should use the sibling
/// [`cpi::accounts`] module (also generated), which re-exports this.
pub(crate) mod __cpi_client_accounts_check_permission {
    use super::*;
    /// Generated CPI struct of the accounts for [`CheckPermission`].
    pub struct CheckPermission<'info> {
        pub sysvar_instructions: anchor_lang::solana_program::account_info::AccountInfo<
            'info,
        >,
        pub permission: anchor_lang::solana_program::account_info::AccountInfo<'info>,
        pub router: anchor_lang::solana_program::account_info::AccountInfo<'info>,
        pub config_router: anchor_lang::solana_program::account_info::AccountInfo<'info>,
        pub express_relay_metadata: anchor_lang::solana_program::account_info::AccountInfo<
            'info,
        >,
    }
    #[automatically_derived]
    impl<'info> anchor_lang::ToAccountMetas for CheckPermission<'info> {
        fn to_account_metas(
            &self,
            is_signer: Option<bool>,
        ) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
            let mut account_metas = ::alloc::vec::Vec::new();
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        anchor_lang::Key::key(&self.sysvar_instructions),
                        false,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        anchor_lang::Key::key(&self.permission),
                        false,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        anchor_lang::Key::key(&self.router),
                        false,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        anchor_lang::Key::key(&self.config_router),
                        false,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        anchor_lang::Key::key(&self.express_relay_metadata),
                        false,
                    ),
                );
            account_metas
        }
    }
    #[automatically_derived]
    impl<'info> anchor_lang::ToAccountInfos<'info> for CheckPermission<'info> {
        fn to_account_infos(
            &self,
        ) -> Vec<anchor_lang::solana_program::account_info::AccountInfo<'info>> {
            let mut account_infos = ::alloc::vec::Vec::new();
            account_infos
                .extend(
                    anchor_lang::ToAccountInfos::to_account_infos(
                        &self.sysvar_instructions,
                    ),
                );
            account_infos
                .extend(anchor_lang::ToAccountInfos::to_account_infos(&self.permission));
            account_infos
                .extend(anchor_lang::ToAccountInfos::to_account_infos(&self.router));
            account_infos
                .extend(
                    anchor_lang::ToAccountInfos::to_account_infos(&self.config_router),
                );
            account_infos
                .extend(
                    anchor_lang::ToAccountInfos::to_account_infos(
                        &self.express_relay_metadata,
                    ),
                );
            account_infos
        }
    }
}
pub struct WithdrawFees<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,
    /// CHECK: this is just the PK where the fees should be sent.
    #[account(mut)]
    pub fee_receiver_admin: UncheckedAccount<'info>,
    #[account(mut, seeds = [SEED_METADATA], bump, has_one = admin)]
    pub express_relay_metadata: Account<'info, ExpressRelayMetadata>,
}
#[automatically_derived]
impl<'info> anchor_lang::Accounts<'info, WithdrawFeesBumps> for WithdrawFees<'info>
where
    'info: 'info,
{
    #[inline(never)]
    fn try_accounts(
        __program_id: &anchor_lang::solana_program::pubkey::Pubkey,
        __accounts: &mut &'info [anchor_lang::solana_program::account_info::AccountInfo<
            'info,
        >],
        __ix_data: &[u8],
        __bumps: &mut WithdrawFeesBumps,
        __reallocs: &mut std::collections::BTreeSet<
            anchor_lang::solana_program::pubkey::Pubkey,
        >,
    ) -> anchor_lang::Result<Self> {
        let admin: Signer = anchor_lang::Accounts::try_accounts(
                __program_id,
                __accounts,
                __ix_data,
                __bumps,
                __reallocs,
            )
            .map_err(|e| e.with_account_name("admin"))?;
        let fee_receiver_admin: UncheckedAccount = anchor_lang::Accounts::try_accounts(
                __program_id,
                __accounts,
                __ix_data,
                __bumps,
                __reallocs,
            )
            .map_err(|e| e.with_account_name("fee_receiver_admin"))?;
        let express_relay_metadata: anchor_lang::accounts::account::Account<
            ExpressRelayMetadata,
        > = anchor_lang::Accounts::try_accounts(
                __program_id,
                __accounts,
                __ix_data,
                __bumps,
                __reallocs,
            )
            .map_err(|e| e.with_account_name("express_relay_metadata"))?;
        if !AsRef::<AccountInfo>::as_ref(&admin).is_writable {
            return Err(
                anchor_lang::error::Error::from(
                        anchor_lang::error::ErrorCode::ConstraintMut,
                    )
                    .with_account_name("admin"),
            );
        }
        if !AsRef::<AccountInfo>::as_ref(&fee_receiver_admin).is_writable {
            return Err(
                anchor_lang::error::Error::from(
                        anchor_lang::error::ErrorCode::ConstraintMut,
                    )
                    .with_account_name("fee_receiver_admin"),
            );
        }
        let (__pda_address, __bump) = Pubkey::find_program_address(
            &[SEED_METADATA],
            &__program_id,
        );
        __bumps.express_relay_metadata = __bump;
        if express_relay_metadata.key() != __pda_address {
            return Err(
                anchor_lang::error::Error::from(
                        anchor_lang::error::ErrorCode::ConstraintSeeds,
                    )
                    .with_account_name("express_relay_metadata")
                    .with_pubkeys((express_relay_metadata.key(), __pda_address)),
            );
        }
        if !AsRef::<AccountInfo>::as_ref(&express_relay_metadata).is_writable {
            return Err(
                anchor_lang::error::Error::from(
                        anchor_lang::error::ErrorCode::ConstraintMut,
                    )
                    .with_account_name("express_relay_metadata"),
            );
        }
        {
            let my_key = express_relay_metadata.admin;
            let target_key = admin.key();
            if my_key != target_key {
                return Err(
                    anchor_lang::error::Error::from(
                            anchor_lang::error::ErrorCode::ConstraintHasOne,
                        )
                        .with_account_name("express_relay_metadata")
                        .with_pubkeys((my_key, target_key)),
                );
            }
        }
        Ok(WithdrawFees {
            admin,
            fee_receiver_admin,
            express_relay_metadata,
        })
    }
}
#[automatically_derived]
impl<'info> anchor_lang::ToAccountInfos<'info> for WithdrawFees<'info>
where
    'info: 'info,
{
    fn to_account_infos(
        &self,
    ) -> Vec<anchor_lang::solana_program::account_info::AccountInfo<'info>> {
        let mut account_infos = ::alloc::vec::Vec::new();
        account_infos.extend(self.admin.to_account_infos());
        account_infos.extend(self.fee_receiver_admin.to_account_infos());
        account_infos.extend(self.express_relay_metadata.to_account_infos());
        account_infos
    }
}
#[automatically_derived]
impl<'info> anchor_lang::ToAccountMetas for WithdrawFees<'info> {
    fn to_account_metas(
        &self,
        is_signer: Option<bool>,
    ) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
        let mut account_metas = ::alloc::vec::Vec::new();
        account_metas.extend(self.admin.to_account_metas(None));
        account_metas.extend(self.fee_receiver_admin.to_account_metas(None));
        account_metas.extend(self.express_relay_metadata.to_account_metas(None));
        account_metas
    }
}
#[automatically_derived]
impl<'info> anchor_lang::AccountsExit<'info> for WithdrawFees<'info>
where
    'info: 'info,
{
    fn exit(
        &self,
        program_id: &anchor_lang::solana_program::pubkey::Pubkey,
    ) -> anchor_lang::Result<()> {
        anchor_lang::AccountsExit::exit(&self.admin, program_id)
            .map_err(|e| e.with_account_name("admin"))?;
        anchor_lang::AccountsExit::exit(&self.fee_receiver_admin, program_id)
            .map_err(|e| e.with_account_name("fee_receiver_admin"))?;
        anchor_lang::AccountsExit::exit(&self.express_relay_metadata, program_id)
            .map_err(|e| e.with_account_name("express_relay_metadata"))?;
        Ok(())
    }
}
pub struct WithdrawFeesBumps {
    pub express_relay_metadata: u8,
}
#[automatically_derived]
impl ::core::fmt::Debug for WithdrawFeesBumps {
    #[inline]
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        ::core::fmt::Formatter::debug_struct_field1_finish(
            f,
            "WithdrawFeesBumps",
            "express_relay_metadata",
            &&self.express_relay_metadata,
        )
    }
}
impl Default for WithdrawFeesBumps {
    fn default() -> Self {
        WithdrawFeesBumps {
            express_relay_metadata: u8::MAX,
        }
    }
}
impl<'info> anchor_lang::Bumps for WithdrawFees<'info>
where
    'info: 'info,
{
    type Bumps = WithdrawFeesBumps;
}
/// An internal, Anchor generated module. This is used (as an
/// implementation detail), to generate a struct for a given
/// `#[derive(Accounts)]` implementation, where each field is a Pubkey,
/// instead of an `AccountInfo`. This is useful for clients that want
/// to generate a list of accounts, without explicitly knowing the
/// order all the fields should be in.
///
/// To access the struct in this module, one should use the sibling
/// `accounts` module (also generated), which re-exports this.
pub(crate) mod __client_accounts_withdraw_fees {
    use super::*;
    use anchor_lang::prelude::borsh;
    /// Generated client accounts for [`WithdrawFees`].
    pub struct WithdrawFees {
        pub admin: Pubkey,
        pub fee_receiver_admin: Pubkey,
        pub express_relay_metadata: Pubkey,
    }
    impl borsh::ser::BorshSerialize for WithdrawFees
    where
        Pubkey: borsh::ser::BorshSerialize,
        Pubkey: borsh::ser::BorshSerialize,
        Pubkey: borsh::ser::BorshSerialize,
    {
        fn serialize<W: borsh::maybestd::io::Write>(
            &self,
            writer: &mut W,
        ) -> ::core::result::Result<(), borsh::maybestd::io::Error> {
            borsh::BorshSerialize::serialize(&self.admin, writer)?;
            borsh::BorshSerialize::serialize(&self.fee_receiver_admin, writer)?;
            borsh::BorshSerialize::serialize(&self.express_relay_metadata, writer)?;
            Ok(())
        }
    }
    #[automatically_derived]
    impl anchor_lang::ToAccountMetas for WithdrawFees {
        fn to_account_metas(
            &self,
            is_signer: Option<bool>,
        ) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
            let mut account_metas = ::alloc::vec::Vec::new();
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        self.admin,
                        true,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        self.fee_receiver_admin,
                        false,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        self.express_relay_metadata,
                        false,
                    ),
                );
            account_metas
        }
    }
}
/// An internal, Anchor generated module. This is used (as an
/// implementation detail), to generate a CPI struct for a given
/// `#[derive(Accounts)]` implementation, where each field is an
/// AccountInfo.
///
/// To access the struct in this module, one should use the sibling
/// [`cpi::accounts`] module (also generated), which re-exports this.
pub(crate) mod __cpi_client_accounts_withdraw_fees {
    use super::*;
    /// Generated CPI struct of the accounts for [`WithdrawFees`].
    pub struct WithdrawFees<'info> {
        pub admin: anchor_lang::solana_program::account_info::AccountInfo<'info>,
        pub fee_receiver_admin: anchor_lang::solana_program::account_info::AccountInfo<
            'info,
        >,
        pub express_relay_metadata: anchor_lang::solana_program::account_info::AccountInfo<
            'info,
        >,
    }
    #[automatically_derived]
    impl<'info> anchor_lang::ToAccountMetas for WithdrawFees<'info> {
        fn to_account_metas(
            &self,
            is_signer: Option<bool>,
        ) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
            let mut account_metas = ::alloc::vec::Vec::new();
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        anchor_lang::Key::key(&self.admin),
                        true,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        anchor_lang::Key::key(&self.fee_receiver_admin),
                        false,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        anchor_lang::Key::key(&self.express_relay_metadata),
                        false,
                    ),
                );
            account_metas
        }
    }
    #[automatically_derived]
    impl<'info> anchor_lang::ToAccountInfos<'info> for WithdrawFees<'info> {
        fn to_account_infos(
            &self,
        ) -> Vec<anchor_lang::solana_program::account_info::AccountInfo<'info>> {
            let mut account_infos = ::alloc::vec::Vec::new();
            account_infos
                .extend(anchor_lang::ToAccountInfos::to_account_infos(&self.admin));
            account_infos
                .extend(
                    anchor_lang::ToAccountInfos::to_account_infos(
                        &self.fee_receiver_admin,
                    ),
                );
            account_infos
                .extend(
                    anchor_lang::ToAccountInfos::to_account_infos(
                        &self.express_relay_metadata,
                    ),
                );
            account_infos
        }
    }
}
pub enum FeeToken {
    Input,
    Output,
}
impl borsh::ser::BorshSerialize for FeeToken {
    fn serialize<W: borsh::maybestd::io::Write>(
        &self,
        writer: &mut W,
    ) -> ::core::result::Result<(), borsh::maybestd::io::Error> {
        let variant_idx: u8 = match self {
            FeeToken::Input => 0u8,
            FeeToken::Output => 1u8,
        };
        writer.write_all(&variant_idx.to_le_bytes())?;
        match self {
            FeeToken::Input => {}
            FeeToken::Output => {}
        }
        Ok(())
    }
}
impl borsh::de::BorshDeserialize for FeeToken {
    fn deserialize_reader<R: borsh::maybestd::io::Read>(
        reader: &mut R,
    ) -> ::core::result::Result<Self, borsh::maybestd::io::Error> {
        let tag = <u8 as borsh::de::BorshDeserialize>::deserialize_reader(reader)?;
        <Self as borsh::de::EnumExt>::deserialize_variant(reader, tag)
    }
}
impl borsh::de::EnumExt for FeeToken {
    fn deserialize_variant<R: borsh::maybestd::io::Read>(
        reader: &mut R,
        variant_idx: u8,
    ) -> ::core::result::Result<Self, borsh::maybestd::io::Error> {
        let mut return_value = match variant_idx {
            0u8 => FeeToken::Input,
            1u8 => FeeToken::Output,
            _ => {
                return Err(
                    borsh::maybestd::io::Error::new(
                        borsh::maybestd::io::ErrorKind::InvalidInput,
                        ::alloc::__export::must_use({
                            let res = ::alloc::fmt::format(
                                format_args!("Unexpected variant index: {0:?}", variant_idx),
                            );
                            res
                        }),
                    ),
                );
            }
        };
        Ok(return_value)
    }
}
#[automatically_derived]
impl ::core::cmp::Eq for FeeToken {
    #[inline]
    #[doc(hidden)]
    #[coverage(off)]
    fn assert_receiver_is_total_eq(&self) -> () {}
}
#[automatically_derived]
impl ::core::marker::StructuralPartialEq for FeeToken {}
#[automatically_derived]
impl ::core::cmp::PartialEq for FeeToken {
    #[inline]
    fn eq(&self, other: &FeeToken) -> bool {
        let __self_discr = ::core::intrinsics::discriminant_value(self);
        let __arg1_discr = ::core::intrinsics::discriminant_value(other);
        __self_discr == __arg1_discr
    }
}
#[automatically_derived]
impl ::core::clone::Clone for FeeToken {
    #[inline]
    fn clone(&self) -> FeeToken {
        *self
    }
}
#[automatically_derived]
impl ::core::marker::Copy for FeeToken {}
#[automatically_derived]
impl ::core::fmt::Debug for FeeToken {
    #[inline]
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        ::core::fmt::Formatter::write_str(
            f,
            match self {
                FeeToken::Input => "Input",
                FeeToken::Output => "Output",
            },
        )
    }
}
/// For all swap instructions and contexts, input and output are defined with respect to the searcher
/// So mint_input refers to the token that the searcher provides to the trader
/// mint_output refers to the token that the searcher receives from the trader
/// This choice is made to minimize confusion for the searchers, who are more likely to parse the program
pub struct SwapArgs {
    pub amount_input: u64,
    pub amount_output: u64,
    pub referral_fee: u64,
    pub fee_token: FeeToken,
}
impl borsh::ser::BorshSerialize for SwapArgs
where
    u64: borsh::ser::BorshSerialize,
    u64: borsh::ser::BorshSerialize,
    u64: borsh::ser::BorshSerialize,
    FeeToken: borsh::ser::BorshSerialize,
{
    fn serialize<W: borsh::maybestd::io::Write>(
        &self,
        writer: &mut W,
    ) -> ::core::result::Result<(), borsh::maybestd::io::Error> {
        borsh::BorshSerialize::serialize(&self.amount_input, writer)?;
        borsh::BorshSerialize::serialize(&self.amount_output, writer)?;
        borsh::BorshSerialize::serialize(&self.referral_fee, writer)?;
        borsh::BorshSerialize::serialize(&self.fee_token, writer)?;
        Ok(())
    }
}
impl borsh::de::BorshDeserialize for SwapArgs
where
    u64: borsh::BorshDeserialize,
    u64: borsh::BorshDeserialize,
    u64: borsh::BorshDeserialize,
    FeeToken: borsh::BorshDeserialize,
{
    fn deserialize_reader<R: borsh::maybestd::io::Read>(
        reader: &mut R,
    ) -> ::core::result::Result<Self, borsh::maybestd::io::Error> {
        Ok(Self {
            amount_input: borsh::BorshDeserialize::deserialize_reader(reader)?,
            amount_output: borsh::BorshDeserialize::deserialize_reader(reader)?,
            referral_fee: borsh::BorshDeserialize::deserialize_reader(reader)?,
            fee_token: borsh::BorshDeserialize::deserialize_reader(reader)?,
        })
    }
}
#[automatically_derived]
impl ::core::cmp::Eq for SwapArgs {
    #[inline]
    #[doc(hidden)]
    #[coverage(off)]
    fn assert_receiver_is_total_eq(&self) -> () {
        let _: ::core::cmp::AssertParamIsEq<u64>;
        let _: ::core::cmp::AssertParamIsEq<FeeToken>;
    }
}
#[automatically_derived]
impl ::core::marker::StructuralPartialEq for SwapArgs {}
#[automatically_derived]
impl ::core::cmp::PartialEq for SwapArgs {
    #[inline]
    fn eq(&self, other: &SwapArgs) -> bool {
        self.amount_input == other.amount_input
            && self.amount_output == other.amount_output
            && self.referral_fee == other.referral_fee
            && self.fee_token == other.fee_token
    }
}
#[automatically_derived]
impl ::core::clone::Clone for SwapArgs {
    #[inline]
    fn clone(&self) -> SwapArgs {
        let _: ::core::clone::AssertParamIsClone<u64>;
        let _: ::core::clone::AssertParamIsClone<FeeToken>;
        *self
    }
}
#[automatically_derived]
impl ::core::marker::Copy for SwapArgs {}
#[automatically_derived]
impl ::core::fmt::Debug for SwapArgs {
    #[inline]
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        ::core::fmt::Formatter::debug_struct_field4_finish(
            f,
            "SwapArgs",
            "amount_input",
            &self.amount_input,
            "amount_output",
            &self.amount_output,
            "referral_fee",
            &self.referral_fee,
            "fee_token",
            &&self.fee_token,
        )
    }
}
pub struct SendFeeArgs<'info> {
    /// Amount to which the fee will be applied
    pub fee_express_relay: u64,
    pub fee_relayer: u64,
    pub fee_router: u64,
    pub from: InterfaceAccount<'info, TokenAccount>,
    pub authority: Signer<'info>,
    pub mint: InterfaceAccount<'info, Mint>,
    pub token_program: Interface<'info, TokenInterface>,
}
#[instruction(data:Box<SwapArgs>)]
pub struct Swap<'info> {
    /// Searcher is the party that sends the input token and receives the output token
    pub searcher: Signer<'info>,
    /// Trader is the party that sends the output token and receives the input token
    pub trader: Signer<'info>,
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
    #[account(mut)]
    pub router_fee_receiver_ata: InterfaceAccount<'info, TokenAccount>,
    #[account(mut, token::authority = express_relay_metadata.relayer_signer)]
    pub relayer_fee_receiver_ata: InterfaceAccount<'info, TokenAccount>,
    #[account(mut, token::authority = express_relay_metadata.key())]
    pub express_relay_fee_receiver_ata: InterfaceAccount<'info, TokenAccount>,
    #[account(mint::token_program = token_program_input)]
    pub mint_input: InterfaceAccount<'info, Mint>,
    #[account(mint::token_program = token_program_output)]
    pub mint_output: InterfaceAccount<'info, Mint>,
    pub token_program_input: Interface<'info, TokenInterface>,
    pub token_program_output: Interface<'info, TokenInterface>,
    /// Express relay configuration
    #[account(seeds = [SEED_METADATA], bump)]
    pub express_relay_metadata: Account<'info, ExpressRelayMetadata>,
}
#[automatically_derived]
impl<'info> anchor_lang::Accounts<'info, SwapBumps> for Swap<'info>
where
    'info: 'info,
{
    #[inline(never)]
    fn try_accounts(
        __program_id: &anchor_lang::solana_program::pubkey::Pubkey,
        __accounts: &mut &'info [anchor_lang::solana_program::account_info::AccountInfo<
            'info,
        >],
        __ix_data: &[u8],
        __bumps: &mut SwapBumps,
        __reallocs: &mut std::collections::BTreeSet<
            anchor_lang::solana_program::pubkey::Pubkey,
        >,
    ) -> anchor_lang::Result<Self> {
        let mut __ix_data = __ix_data;
        struct __Args {
            data: Box<SwapArgs>,
        }
        impl borsh::ser::BorshSerialize for __Args
        where
            Box<SwapArgs>: borsh::ser::BorshSerialize,
        {
            fn serialize<W: borsh::maybestd::io::Write>(
                &self,
                writer: &mut W,
            ) -> ::core::result::Result<(), borsh::maybestd::io::Error> {
                borsh::BorshSerialize::serialize(&self.data, writer)?;
                Ok(())
            }
        }
        impl borsh::de::BorshDeserialize for __Args
        where
            Box<SwapArgs>: borsh::BorshDeserialize,
        {
            fn deserialize_reader<R: borsh::maybestd::io::Read>(
                reader: &mut R,
            ) -> ::core::result::Result<Self, borsh::maybestd::io::Error> {
                Ok(Self {
                    data: borsh::BorshDeserialize::deserialize_reader(reader)?,
                })
            }
        }
        let __Args { data } = __Args::deserialize(&mut __ix_data)
            .map_err(|_| anchor_lang::error::ErrorCode::InstructionDidNotDeserialize)?;
        let searcher: Signer = anchor_lang::Accounts::try_accounts(
                __program_id,
                __accounts,
                __ix_data,
                __bumps,
                __reallocs,
            )
            .map_err(|e| e.with_account_name("searcher"))?;
        let trader: Signer = anchor_lang::Accounts::try_accounts(
                __program_id,
                __accounts,
                __ix_data,
                __bumps,
                __reallocs,
            )
            .map_err(|e| e.with_account_name("trader"))?;
        let searcher_input_ta: anchor_lang::accounts::interface_account::InterfaceAccount<
            TokenAccount,
        > = anchor_lang::Accounts::try_accounts(
                __program_id,
                __accounts,
                __ix_data,
                __bumps,
                __reallocs,
            )
            .map_err(|e| e.with_account_name("searcher_input_ta"))?;
        let searcher_output_ta: anchor_lang::accounts::interface_account::InterfaceAccount<
            TokenAccount,
        > = anchor_lang::Accounts::try_accounts(
                __program_id,
                __accounts,
                __ix_data,
                __bumps,
                __reallocs,
            )
            .map_err(|e| e.with_account_name("searcher_output_ta"))?;
        let trader_input_ata: anchor_lang::accounts::interface_account::InterfaceAccount<
            TokenAccount,
        > = anchor_lang::Accounts::try_accounts(
                __program_id,
                __accounts,
                __ix_data,
                __bumps,
                __reallocs,
            )
            .map_err(|e| e.with_account_name("trader_input_ata"))?;
        let trader_output_ata: anchor_lang::accounts::interface_account::InterfaceAccount<
            TokenAccount,
        > = anchor_lang::Accounts::try_accounts(
                __program_id,
                __accounts,
                __ix_data,
                __bumps,
                __reallocs,
            )
            .map_err(|e| e.with_account_name("trader_output_ata"))?;
        let router_fee_receiver_ata: anchor_lang::accounts::interface_account::InterfaceAccount<
            TokenAccount,
        > = anchor_lang::Accounts::try_accounts(
                __program_id,
                __accounts,
                __ix_data,
                __bumps,
                __reallocs,
            )
            .map_err(|e| e.with_account_name("router_fee_receiver_ata"))?;
        let relayer_fee_receiver_ata: anchor_lang::accounts::interface_account::InterfaceAccount<
            TokenAccount,
        > = anchor_lang::Accounts::try_accounts(
                __program_id,
                __accounts,
                __ix_data,
                __bumps,
                __reallocs,
            )
            .map_err(|e| e.with_account_name("relayer_fee_receiver_ata"))?;
        let express_relay_fee_receiver_ata: anchor_lang::accounts::interface_account::InterfaceAccount<
            TokenAccount,
        > = anchor_lang::Accounts::try_accounts(
                __program_id,
                __accounts,
                __ix_data,
                __bumps,
                __reallocs,
            )
            .map_err(|e| e.with_account_name("express_relay_fee_receiver_ata"))?;
        let mint_input: anchor_lang::accounts::interface_account::InterfaceAccount<
            Mint,
        > = anchor_lang::Accounts::try_accounts(
                __program_id,
                __accounts,
                __ix_data,
                __bumps,
                __reallocs,
            )
            .map_err(|e| e.with_account_name("mint_input"))?;
        let mint_output: anchor_lang::accounts::interface_account::InterfaceAccount<
            Mint,
        > = anchor_lang::Accounts::try_accounts(
                __program_id,
                __accounts,
                __ix_data,
                __bumps,
                __reallocs,
            )
            .map_err(|e| e.with_account_name("mint_output"))?;
        let token_program_input: anchor_lang::accounts::interface::Interface<
            TokenInterface,
        > = anchor_lang::Accounts::try_accounts(
                __program_id,
                __accounts,
                __ix_data,
                __bumps,
                __reallocs,
            )
            .map_err(|e| e.with_account_name("token_program_input"))?;
        let token_program_output: anchor_lang::accounts::interface::Interface<
            TokenInterface,
        > = anchor_lang::Accounts::try_accounts(
                __program_id,
                __accounts,
                __ix_data,
                __bumps,
                __reallocs,
            )
            .map_err(|e| e.with_account_name("token_program_output"))?;
        let express_relay_metadata: anchor_lang::accounts::account::Account<
            ExpressRelayMetadata,
        > = anchor_lang::Accounts::try_accounts(
                __program_id,
                __accounts,
                __ix_data,
                __bumps,
                __reallocs,
            )
            .map_err(|e| e.with_account_name("express_relay_metadata"))?;
        if !AsRef::<AccountInfo>::as_ref(&searcher_input_ta).is_writable {
            return Err(
                anchor_lang::error::Error::from(
                        anchor_lang::error::ErrorCode::ConstraintMut,
                    )
                    .with_account_name("searcher_input_ta"),
            );
        }
        {
            if searcher_input_ta.owner != searcher.key() {
                return Err(anchor_lang::error::ErrorCode::ConstraintTokenOwner.into());
            }
            if searcher_input_ta.mint != mint_input.key() {
                return Err(anchor_lang::error::ErrorCode::ConstraintTokenMint.into());
            }
            if AsRef::<AccountInfo>::as_ref(&searcher_input_ta).owner
                != &token_program_input.key()
            {
                return Err(
                    anchor_lang::error::ErrorCode::ConstraintTokenTokenProgram.into(),
                );
            }
        }
        if !AsRef::<AccountInfo>::as_ref(&searcher_output_ta).is_writable {
            return Err(
                anchor_lang::error::Error::from(
                        anchor_lang::error::ErrorCode::ConstraintMut,
                    )
                    .with_account_name("searcher_output_ta"),
            );
        }
        {
            if searcher_output_ta.owner != searcher.key() {
                return Err(anchor_lang::error::ErrorCode::ConstraintTokenOwner.into());
            }
            if searcher_output_ta.mint != mint_output.key() {
                return Err(anchor_lang::error::ErrorCode::ConstraintTokenMint.into());
            }
            if AsRef::<AccountInfo>::as_ref(&searcher_output_ta).owner
                != &token_program_output.key()
            {
                return Err(
                    anchor_lang::error::ErrorCode::ConstraintTokenTokenProgram.into(),
                );
            }
        }
        {
            if AsRef::<AccountInfo>::as_ref(&trader_input_ata).owner
                != &token_program_input.key()
            {
                return Err(
                    anchor_lang::error::ErrorCode::ConstraintAssociatedTokenTokenProgram
                        .into(),
                );
            }
            let my_owner = trader_input_ata.owner;
            let wallet_address = trader.key();
            if my_owner != wallet_address {
                return Err(
                    anchor_lang::error::Error::from(
                            anchor_lang::error::ErrorCode::ConstraintTokenOwner,
                        )
                        .with_account_name("trader_input_ata")
                        .with_pubkeys((my_owner, wallet_address)),
                );
            }
            let __associated_token_address = ::anchor_spl::associated_token::get_associated_token_address_with_program_id(
                &wallet_address,
                &mint_input.key(),
                &token_program_input.key(),
            );
            let my_key = trader_input_ata.key();
            if my_key != __associated_token_address {
                return Err(
                    anchor_lang::error::Error::from(
                            anchor_lang::error::ErrorCode::ConstraintAssociated,
                        )
                        .with_account_name("trader_input_ata")
                        .with_pubkeys((my_key, __associated_token_address)),
                );
            }
        }
        if !AsRef::<AccountInfo>::as_ref(&trader_input_ata).is_writable {
            return Err(
                anchor_lang::error::Error::from(
                        anchor_lang::error::ErrorCode::ConstraintMut,
                    )
                    .with_account_name("trader_input_ata"),
            );
        }
        {
            if AsRef::<AccountInfo>::as_ref(&trader_output_ata).owner
                != &token_program_output.key()
            {
                return Err(
                    anchor_lang::error::ErrorCode::ConstraintAssociatedTokenTokenProgram
                        .into(),
                );
            }
            let my_owner = trader_output_ata.owner;
            let wallet_address = trader.key();
            if my_owner != wallet_address {
                return Err(
                    anchor_lang::error::Error::from(
                            anchor_lang::error::ErrorCode::ConstraintTokenOwner,
                        )
                        .with_account_name("trader_output_ata")
                        .with_pubkeys((my_owner, wallet_address)),
                );
            }
            let __associated_token_address = ::anchor_spl::associated_token::get_associated_token_address_with_program_id(
                &wallet_address,
                &mint_output.key(),
                &token_program_output.key(),
            );
            let my_key = trader_output_ata.key();
            if my_key != __associated_token_address {
                return Err(
                    anchor_lang::error::Error::from(
                            anchor_lang::error::ErrorCode::ConstraintAssociated,
                        )
                        .with_account_name("trader_output_ata")
                        .with_pubkeys((my_key, __associated_token_address)),
                );
            }
        }
        if !AsRef::<AccountInfo>::as_ref(&trader_output_ata).is_writable {
            return Err(
                anchor_lang::error::Error::from(
                        anchor_lang::error::ErrorCode::ConstraintMut,
                    )
                    .with_account_name("trader_output_ata"),
            );
        }
        if !AsRef::<AccountInfo>::as_ref(&router_fee_receiver_ata).is_writable {
            return Err(
                anchor_lang::error::Error::from(
                        anchor_lang::error::ErrorCode::ConstraintMut,
                    )
                    .with_account_name("router_fee_receiver_ata"),
            );
        }
        if !AsRef::<AccountInfo>::as_ref(&relayer_fee_receiver_ata).is_writable {
            return Err(
                anchor_lang::error::Error::from(
                        anchor_lang::error::ErrorCode::ConstraintMut,
                    )
                    .with_account_name("relayer_fee_receiver_ata"),
            );
        }
        {
            if relayer_fee_receiver_ata.owner
                != express_relay_metadata.relayer_signer.key()
            {
                return Err(anchor_lang::error::ErrorCode::ConstraintTokenOwner.into());
            }
        }
        if !AsRef::<AccountInfo>::as_ref(&express_relay_fee_receiver_ata).is_writable {
            return Err(
                anchor_lang::error::Error::from(
                        anchor_lang::error::ErrorCode::ConstraintMut,
                    )
                    .with_account_name("express_relay_fee_receiver_ata"),
            );
        }
        {
            if express_relay_fee_receiver_ata.owner != express_relay_metadata.key().key()
            {
                return Err(anchor_lang::error::ErrorCode::ConstraintTokenOwner.into());
            }
        }
        {
            if AsRef::<AccountInfo>::as_ref(&mint_input).owner
                != &token_program_input.key()
            {
                return Err(
                    anchor_lang::error::ErrorCode::ConstraintMintTokenProgram.into(),
                );
            }
        }
        {
            if AsRef::<AccountInfo>::as_ref(&mint_output).owner
                != &token_program_output.key()
            {
                return Err(
                    anchor_lang::error::ErrorCode::ConstraintMintTokenProgram.into(),
                );
            }
        }
        let (__pda_address, __bump) = Pubkey::find_program_address(
            &[SEED_METADATA],
            &__program_id,
        );
        __bumps.express_relay_metadata = __bump;
        if express_relay_metadata.key() != __pda_address {
            return Err(
                anchor_lang::error::Error::from(
                        anchor_lang::error::ErrorCode::ConstraintSeeds,
                    )
                    .with_account_name("express_relay_metadata")
                    .with_pubkeys((express_relay_metadata.key(), __pda_address)),
            );
        }
        Ok(Swap {
            searcher,
            trader,
            searcher_input_ta,
            searcher_output_ta,
            trader_input_ata,
            trader_output_ata,
            router_fee_receiver_ata,
            relayer_fee_receiver_ata,
            express_relay_fee_receiver_ata,
            mint_input,
            mint_output,
            token_program_input,
            token_program_output,
            express_relay_metadata,
        })
    }
}
#[automatically_derived]
impl<'info> anchor_lang::ToAccountInfos<'info> for Swap<'info>
where
    'info: 'info,
{
    fn to_account_infos(
        &self,
    ) -> Vec<anchor_lang::solana_program::account_info::AccountInfo<'info>> {
        let mut account_infos = ::alloc::vec::Vec::new();
        account_infos.extend(self.searcher.to_account_infos());
        account_infos.extend(self.trader.to_account_infos());
        account_infos.extend(self.searcher_input_ta.to_account_infos());
        account_infos.extend(self.searcher_output_ta.to_account_infos());
        account_infos.extend(self.trader_input_ata.to_account_infos());
        account_infos.extend(self.trader_output_ata.to_account_infos());
        account_infos.extend(self.router_fee_receiver_ata.to_account_infos());
        account_infos.extend(self.relayer_fee_receiver_ata.to_account_infos());
        account_infos.extend(self.express_relay_fee_receiver_ata.to_account_infos());
        account_infos.extend(self.mint_input.to_account_infos());
        account_infos.extend(self.mint_output.to_account_infos());
        account_infos.extend(self.token_program_input.to_account_infos());
        account_infos.extend(self.token_program_output.to_account_infos());
        account_infos.extend(self.express_relay_metadata.to_account_infos());
        account_infos
    }
}
#[automatically_derived]
impl<'info> anchor_lang::ToAccountMetas for Swap<'info> {
    fn to_account_metas(
        &self,
        is_signer: Option<bool>,
    ) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
        let mut account_metas = ::alloc::vec::Vec::new();
        account_metas.extend(self.searcher.to_account_metas(None));
        account_metas.extend(self.trader.to_account_metas(None));
        account_metas.extend(self.searcher_input_ta.to_account_metas(None));
        account_metas.extend(self.searcher_output_ta.to_account_metas(None));
        account_metas.extend(self.trader_input_ata.to_account_metas(None));
        account_metas.extend(self.trader_output_ata.to_account_metas(None));
        account_metas.extend(self.router_fee_receiver_ata.to_account_metas(None));
        account_metas.extend(self.relayer_fee_receiver_ata.to_account_metas(None));
        account_metas.extend(self.express_relay_fee_receiver_ata.to_account_metas(None));
        account_metas.extend(self.mint_input.to_account_metas(None));
        account_metas.extend(self.mint_output.to_account_metas(None));
        account_metas.extend(self.token_program_input.to_account_metas(None));
        account_metas.extend(self.token_program_output.to_account_metas(None));
        account_metas.extend(self.express_relay_metadata.to_account_metas(None));
        account_metas
    }
}
#[automatically_derived]
impl<'info> anchor_lang::AccountsExit<'info> for Swap<'info>
where
    'info: 'info,
{
    fn exit(
        &self,
        program_id: &anchor_lang::solana_program::pubkey::Pubkey,
    ) -> anchor_lang::Result<()> {
        anchor_lang::AccountsExit::exit(&self.searcher_input_ta, program_id)
            .map_err(|e| e.with_account_name("searcher_input_ta"))?;
        anchor_lang::AccountsExit::exit(&self.searcher_output_ta, program_id)
            .map_err(|e| e.with_account_name("searcher_output_ta"))?;
        anchor_lang::AccountsExit::exit(&self.trader_input_ata, program_id)
            .map_err(|e| e.with_account_name("trader_input_ata"))?;
        anchor_lang::AccountsExit::exit(&self.trader_output_ata, program_id)
            .map_err(|e| e.with_account_name("trader_output_ata"))?;
        anchor_lang::AccountsExit::exit(&self.router_fee_receiver_ata, program_id)
            .map_err(|e| e.with_account_name("router_fee_receiver_ata"))?;
        anchor_lang::AccountsExit::exit(&self.relayer_fee_receiver_ata, program_id)
            .map_err(|e| e.with_account_name("relayer_fee_receiver_ata"))?;
        anchor_lang::AccountsExit::exit(&self.express_relay_fee_receiver_ata, program_id)
            .map_err(|e| e.with_account_name("express_relay_fee_receiver_ata"))?;
        Ok(())
    }
}
pub struct SwapBumps {
    pub express_relay_metadata: u8,
}
#[automatically_derived]
impl ::core::fmt::Debug for SwapBumps {
    #[inline]
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        ::core::fmt::Formatter::debug_struct_field1_finish(
            f,
            "SwapBumps",
            "express_relay_metadata",
            &&self.express_relay_metadata,
        )
    }
}
impl Default for SwapBumps {
    fn default() -> Self {
        SwapBumps {
            express_relay_metadata: u8::MAX,
        }
    }
}
impl<'info> anchor_lang::Bumps for Swap<'info>
where
    'info: 'info,
{
    type Bumps = SwapBumps;
}
/// An internal, Anchor generated module. This is used (as an
/// implementation detail), to generate a struct for a given
/// `#[derive(Accounts)]` implementation, where each field is a Pubkey,
/// instead of an `AccountInfo`. This is useful for clients that want
/// to generate a list of accounts, without explicitly knowing the
/// order all the fields should be in.
///
/// To access the struct in this module, one should use the sibling
/// `accounts` module (also generated), which re-exports this.
pub(crate) mod __client_accounts_swap {
    use super::*;
    use anchor_lang::prelude::borsh;
    /// Generated client accounts for [`Swap`].
    pub struct Swap {
        ///Searcher is the party that sends the input token and receives the output token
        pub searcher: Pubkey,
        ///Trader is the party that sends the output token and receives the input token
        pub trader: Pubkey,
        pub searcher_input_ta: Pubkey,
        pub searcher_output_ta: Pubkey,
        pub trader_input_ata: Pubkey,
        pub trader_output_ata: Pubkey,
        pub router_fee_receiver_ata: Pubkey,
        pub relayer_fee_receiver_ata: Pubkey,
        pub express_relay_fee_receiver_ata: Pubkey,
        pub mint_input: Pubkey,
        pub mint_output: Pubkey,
        pub token_program_input: Pubkey,
        pub token_program_output: Pubkey,
        ///Express relay configuration
        pub express_relay_metadata: Pubkey,
    }
    impl borsh::ser::BorshSerialize for Swap
    where
        Pubkey: borsh::ser::BorshSerialize,
        Pubkey: borsh::ser::BorshSerialize,
        Pubkey: borsh::ser::BorshSerialize,
        Pubkey: borsh::ser::BorshSerialize,
        Pubkey: borsh::ser::BorshSerialize,
        Pubkey: borsh::ser::BorshSerialize,
        Pubkey: borsh::ser::BorshSerialize,
        Pubkey: borsh::ser::BorshSerialize,
        Pubkey: borsh::ser::BorshSerialize,
        Pubkey: borsh::ser::BorshSerialize,
        Pubkey: borsh::ser::BorshSerialize,
        Pubkey: borsh::ser::BorshSerialize,
        Pubkey: borsh::ser::BorshSerialize,
        Pubkey: borsh::ser::BorshSerialize,
    {
        fn serialize<W: borsh::maybestd::io::Write>(
            &self,
            writer: &mut W,
        ) -> ::core::result::Result<(), borsh::maybestd::io::Error> {
            borsh::BorshSerialize::serialize(&self.searcher, writer)?;
            borsh::BorshSerialize::serialize(&self.trader, writer)?;
            borsh::BorshSerialize::serialize(&self.searcher_input_ta, writer)?;
            borsh::BorshSerialize::serialize(&self.searcher_output_ta, writer)?;
            borsh::BorshSerialize::serialize(&self.trader_input_ata, writer)?;
            borsh::BorshSerialize::serialize(&self.trader_output_ata, writer)?;
            borsh::BorshSerialize::serialize(&self.router_fee_receiver_ata, writer)?;
            borsh::BorshSerialize::serialize(&self.relayer_fee_receiver_ata, writer)?;
            borsh::BorshSerialize::serialize(
                &self.express_relay_fee_receiver_ata,
                writer,
            )?;
            borsh::BorshSerialize::serialize(&self.mint_input, writer)?;
            borsh::BorshSerialize::serialize(&self.mint_output, writer)?;
            borsh::BorshSerialize::serialize(&self.token_program_input, writer)?;
            borsh::BorshSerialize::serialize(&self.token_program_output, writer)?;
            borsh::BorshSerialize::serialize(&self.express_relay_metadata, writer)?;
            Ok(())
        }
    }
    #[automatically_derived]
    impl anchor_lang::ToAccountMetas for Swap {
        fn to_account_metas(
            &self,
            is_signer: Option<bool>,
        ) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
            let mut account_metas = ::alloc::vec::Vec::new();
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        self.searcher,
                        true,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        self.trader,
                        true,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        self.searcher_input_ta,
                        false,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        self.searcher_output_ta,
                        false,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        self.trader_input_ata,
                        false,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        self.trader_output_ata,
                        false,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        self.router_fee_receiver_ata,
                        false,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        self.relayer_fee_receiver_ata,
                        false,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        self.express_relay_fee_receiver_ata,
                        false,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        self.mint_input,
                        false,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        self.mint_output,
                        false,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        self.token_program_input,
                        false,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        self.token_program_output,
                        false,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        self.express_relay_metadata,
                        false,
                    ),
                );
            account_metas
        }
    }
}
/// An internal, Anchor generated module. This is used (as an
/// implementation detail), to generate a CPI struct for a given
/// `#[derive(Accounts)]` implementation, where each field is an
/// AccountInfo.
///
/// To access the struct in this module, one should use the sibling
/// [`cpi::accounts`] module (also generated), which re-exports this.
pub(crate) mod __cpi_client_accounts_swap {
    use super::*;
    /// Generated CPI struct of the accounts for [`Swap`].
    pub struct Swap<'info> {
        ///Searcher is the party that sends the input token and receives the output token
        pub searcher: anchor_lang::solana_program::account_info::AccountInfo<'info>,
        ///Trader is the party that sends the output token and receives the input token
        pub trader: anchor_lang::solana_program::account_info::AccountInfo<'info>,
        pub searcher_input_ta: anchor_lang::solana_program::account_info::AccountInfo<
            'info,
        >,
        pub searcher_output_ta: anchor_lang::solana_program::account_info::AccountInfo<
            'info,
        >,
        pub trader_input_ata: anchor_lang::solana_program::account_info::AccountInfo<
            'info,
        >,
        pub trader_output_ata: anchor_lang::solana_program::account_info::AccountInfo<
            'info,
        >,
        pub router_fee_receiver_ata: anchor_lang::solana_program::account_info::AccountInfo<
            'info,
        >,
        pub relayer_fee_receiver_ata: anchor_lang::solana_program::account_info::AccountInfo<
            'info,
        >,
        pub express_relay_fee_receiver_ata: anchor_lang::solana_program::account_info::AccountInfo<
            'info,
        >,
        pub mint_input: anchor_lang::solana_program::account_info::AccountInfo<'info>,
        pub mint_output: anchor_lang::solana_program::account_info::AccountInfo<'info>,
        pub token_program_input: anchor_lang::solana_program::account_info::AccountInfo<
            'info,
        >,
        pub token_program_output: anchor_lang::solana_program::account_info::AccountInfo<
            'info,
        >,
        ///Express relay configuration
        pub express_relay_metadata: anchor_lang::solana_program::account_info::AccountInfo<
            'info,
        >,
    }
    #[automatically_derived]
    impl<'info> anchor_lang::ToAccountMetas for Swap<'info> {
        fn to_account_metas(
            &self,
            is_signer: Option<bool>,
        ) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
            let mut account_metas = ::alloc::vec::Vec::new();
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        anchor_lang::Key::key(&self.searcher),
                        true,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        anchor_lang::Key::key(&self.trader),
                        true,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        anchor_lang::Key::key(&self.searcher_input_ta),
                        false,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        anchor_lang::Key::key(&self.searcher_output_ta),
                        false,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        anchor_lang::Key::key(&self.trader_input_ata),
                        false,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        anchor_lang::Key::key(&self.trader_output_ata),
                        false,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        anchor_lang::Key::key(&self.router_fee_receiver_ata),
                        false,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        anchor_lang::Key::key(&self.relayer_fee_receiver_ata),
                        false,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new(
                        anchor_lang::Key::key(&self.express_relay_fee_receiver_ata),
                        false,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        anchor_lang::Key::key(&self.mint_input),
                        false,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        anchor_lang::Key::key(&self.mint_output),
                        false,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        anchor_lang::Key::key(&self.token_program_input),
                        false,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        anchor_lang::Key::key(&self.token_program_output),
                        false,
                    ),
                );
            account_metas
                .push(
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        anchor_lang::Key::key(&self.express_relay_metadata),
                        false,
                    ),
                );
            account_metas
        }
    }
    #[automatically_derived]
    impl<'info> anchor_lang::ToAccountInfos<'info> for Swap<'info> {
        fn to_account_infos(
            &self,
        ) -> Vec<anchor_lang::solana_program::account_info::AccountInfo<'info>> {
            let mut account_infos = ::alloc::vec::Vec::new();
            account_infos
                .extend(anchor_lang::ToAccountInfos::to_account_infos(&self.searcher));
            account_infos
                .extend(anchor_lang::ToAccountInfos::to_account_infos(&self.trader));
            account_infos
                .extend(
                    anchor_lang::ToAccountInfos::to_account_infos(
                        &self.searcher_input_ta,
                    ),
                );
            account_infos
                .extend(
                    anchor_lang::ToAccountInfos::to_account_infos(
                        &self.searcher_output_ta,
                    ),
                );
            account_infos
                .extend(
                    anchor_lang::ToAccountInfos::to_account_infos(&self.trader_input_ata),
                );
            account_infos
                .extend(
                    anchor_lang::ToAccountInfos::to_account_infos(
                        &self.trader_output_ata,
                    ),
                );
            account_infos
                .extend(
                    anchor_lang::ToAccountInfos::to_account_infos(
                        &self.router_fee_receiver_ata,
                    ),
                );
            account_infos
                .extend(
                    anchor_lang::ToAccountInfos::to_account_infos(
                        &self.relayer_fee_receiver_ata,
                    ),
                );
            account_infos
                .extend(
                    anchor_lang::ToAccountInfos::to_account_infos(
                        &self.express_relay_fee_receiver_ata,
                    ),
                );
            account_infos
                .extend(anchor_lang::ToAccountInfos::to_account_infos(&self.mint_input));
            account_infos
                .extend(
                    anchor_lang::ToAccountInfos::to_account_infos(&self.mint_output),
                );
            account_infos
                .extend(
                    anchor_lang::ToAccountInfos::to_account_infos(
                        &self.token_program_input,
                    ),
                );
            account_infos
                .extend(
                    anchor_lang::ToAccountInfos::to_account_infos(
                        &self.token_program_output,
                    ),
                );
            account_infos
                .extend(
                    anchor_lang::ToAccountInfos::to_account_infos(
                        &self.express_relay_metadata,
                    ),
                );
            account_infos
        }
    }
}
