use {
    crate::{
        bid::BidId,
        AccessLevel,
        ChainId,
        PermissionKeyEvm,
        Routable,
    },
    ethers::types::{
        Address,
        Bytes,
        Signature,
        U256,
    },
    serde::{
        de,
        Deserialize,
        Serialize,
    },
    serde_with::{
        base64::Base64,
        serde_as,
        DisplayFromStr,
    },
    solana_sdk::{
        clock::Slot,
        pubkey::Pubkey,
        transaction::VersionedTransaction,
    },
    strum::{
        AsRefStr,
        Display,
    },
    time::OffsetDateTime,
    utoipa::{
        IntoParams,
        ToResponse,
        ToSchema,
    },
    uuid::Uuid,
};

// Base types
pub type UnixTimestampMicros = i128;
pub type OpportunityId = Uuid;

#[derive(Debug, Serialize, Deserialize, ToSchema, Clone)]
#[serde(rename_all = "lowercase")]
pub enum OpportunityMode {
    Live,
    Historical,
}

#[derive(Serialize, Deserialize, ToResponse, ToSchema, Clone)]
pub struct OpportunityBidResult {
    #[schema(example = "OK")]
    pub status: String,
    /// The unique id created to identify the bid. This id can be used to query the status of the bid.
    #[schema(example = "beedbeed-58cc-4372-a567-0e02b2c3d479", value_type=String)]
    pub id:     BidId,
}

#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug, Display)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum ProgramSvm {
    Swap,
    Limo,
}

/// Opportunity parameters needed for deleting live opportunities.
#[serde_as]
#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug)]
pub struct OpportunityDeleteV1Svm {
    /// The permission account for the opportunity.
    #[schema(example = "DUcTi3rDyS5QEmZ4BNRBejtArmDCWaPYGfN44vBJXKL5", value_type = String)]
    #[serde_as(as = "DisplayFromStr")]
    pub permission_account: Pubkey,
    /// The router account for the opportunity.
    #[schema(example = "DUcTi3rDyS5QEmZ4BNRBejtArmDCWaPYGfN44vBJXKL5", value_type = String)]
    #[serde_as(as = "DisplayFromStr")]
    pub router:             Pubkey,
    /// The chain id for the opportunity.
    #[schema(example = "solana", value_type = String)]
    pub chain_id:           ChainId,
    /// The program for the opportunity.
    #[schema(example = "limo", value_type = ProgramSvm)]
    pub program:            ProgramSvm,
}

/// Opportunity parameters needed for deleting live opportunities.
#[serde_as]
#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug)]
pub struct OpportunityDeleteV1Evm {
    /// The permission key of the opportunity.
    #[schema(example = "0xdeadbeefcafe", value_type = String)]
    pub permission_key: PermissionKeyEvm,
    /// The chain id for the opportunity.
    #[schema(example = "solana", value_type = String)]
    pub chain_id:       ChainId,
}

#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug)]
#[serde(tag = "version")]
pub enum OpportunityDeleteSvm {
    #[serde(rename = "v1")]
    #[schema(title = "v1")]
    V1(OpportunityDeleteV1Svm),
}

#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug)]
#[serde(tag = "version")]
pub enum OpportunityDeleteEvm {
    #[serde(rename = "v1")]
    #[schema(title = "v1")]
    V1(OpportunityDeleteV1Evm),
}

/// The input type for deleting opportunities.
#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug)]
#[serde(tag = "chain_type")]
pub enum OpportunityDelete {
    #[serde(rename = "svm")]
    #[schema(title = "svm")]
    Svm(OpportunityDeleteSvm),
    #[serde(rename = "evm")]
    #[schema(title = "evm")]
    Evm(OpportunityDeleteEvm),
}

#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug, ToResponse)]
pub struct TokenAmountEvm {
    /// The token contract address.
    #[schema(example = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2", value_type = String)]
    pub token:  Address,
    /// The token amount.
    #[schema(example = "1000", value_type = String)]
    #[serde(with = "crate::serde::u256")]
    pub amount: U256,
}

/// Opportunity parameters needed for on-chain execution.
/// If a searcher signs the opportunity and have approved enough tokens to opportunity adapter,
/// by calling this target contract with the given target calldata and structures, they will
/// send the tokens specified in the `sell_tokens` field and receive the tokens specified in the `buy_tokens` field.
#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug, ToResponse)]
pub struct OpportunityCreateV1Evm {
    /// The permission key required for successful execution of the opportunity.
    #[schema(example = "0xdeadbeefcafe", value_type = String)]
    pub permission_key:    PermissionKeyEvm,
    /// The chain id where the opportunity will be executed.
    #[schema(example = "op_sepolia", value_type = String)]
    pub chain_id:          String,
    /// The contract address to call for execution of the opportunity.
    #[schema(example = "0xcA11bde05977b3631167028862bE2a173976CA11", value_type = String)]
    pub target_contract:   ethers::abi::Address,
    /// Calldata for the target contract call.
    #[schema(example = "0xdeadbeef", value_type = String)]
    pub target_calldata:   Bytes,
    /// The value to send with the contract call.
    #[schema(example = "1", value_type = String)]
    #[serde(with = "crate::serde::u256")]
    pub target_call_value: U256,

    pub sell_tokens: Vec<TokenAmountEvm>,
    pub buy_tokens:  Vec<TokenAmountEvm>,
}

#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug, ToResponse)]
#[serde(tag = "version")]
pub enum OpportunityCreateEvm {
    #[serde(rename = "v1")]
    #[schema(title = "v1")]
    V1(OpportunityCreateV1Evm),
}

// ----- Svm types -----
#[serde_as]
#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug, ToResponse)]
pub struct TokenAmountSvm {
    /// The token mint address.
    #[schema(example = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v", value_type = String)]
    #[serde_as(as = "DisplayFromStr")]
    pub token:  Pubkey,
    /// The token amount, represented in the smallest denomination of that token
    /// (e.g. lamports for SOL).
    #[schema(example = 1000)]
    pub amount: u64,
}

/// Program specific parameters for the opportunity.
#[serde_as]
#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug)]
#[serde(tag = "program", rename_all = "snake_case")]
pub enum OpportunityCreateProgramParamsV1Svm {
    /// Limo program specific parameters for the opportunity.
    /// It contains the Limo order to be executed, encoded in base64.
    /// SDKs will decode this order and create transaction for bidding on the opportunity.
    #[schema(title = "limo")]
    Limo {
        /// The Limo order to be executed, encoded in base64.
        #[schema(example = "UxMUbQAsjrfQUp5stVwMJ6Mucq7VWTvt4ICe69BJ8lVXqwM+0sysV8OqZTdM0W4p...", value_type = String)]
        #[serde_as(as = "Base64")]
        order: Vec<u8>,

        /// Address of the order account.
        #[schema(example = "DUcTi3rDyS5QEmZ4BNRBejtArmDCWaPYGfN44vBJXKL5", value_type = String)]
        #[serde_as(as = "DisplayFromStr")]
        order_address: Pubkey,
    },
}

/// Opportunity parameters needed for on-chain execution.
/// Parameters may differ for each program.
#[serde_as]
#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug)]
pub struct OpportunityCreateV1Svm {
    /// The permission account to be permitted by the ER contract for the opportunity execution of the protocol.
    #[schema(example = "DUcTi3rDyS5QEmZ4BNRBejtArmDCWaPYGfN44vBJXKL5", value_type = String)]
    #[serde_as(as = "DisplayFromStr")]
    pub permission_account: Pubkey,
    /// The router account to be used for the opportunity execution of the protocol.
    #[schema(example = "DUcTi3rDyS5QEmZ4BNRBejtArmDCWaPYGfN44vBJXKL5", value_type = String)]
    #[serde_as(as = "DisplayFromStr")]
    pub router:             Pubkey,
    /// The chain id where the opportunity will be executed.
    #[schema(example = "solana", value_type = String)]
    pub chain_id:           ChainId,
    /// The slot where the program params were fetched from using the RPC.
    #[schema(example = 293106477, value_type = u64)]
    pub slot:               Slot,

    pub sell_tokens: Vec<TokenAmountSvm>,
    pub buy_tokens:  Vec<TokenAmountSvm>,

    #[serde(flatten)]
    #[schema(inline)]
    pub program_params: OpportunityCreateProgramParamsV1Svm,
}

#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug)]
#[serde(tag = "version")]
pub enum OpportunityCreateSvm {
    #[serde(rename = "v1")]
    #[schema(title = "v1")]
    V1(OpportunityCreateV1Svm),
}

/// The input type for creating a new opportunity.
#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug)]
#[serde(untagged)]
#[allow(clippy::large_enum_variant)]
pub enum OpportunityCreate {
    #[schema(title = "evm")]
    Evm(OpportunityCreateEvm),
    #[schema(title = "svm")]
    Svm(OpportunityCreateSvm),
}

#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug, ToResponse)]
pub struct OpportunityParamsV1Evm(pub OpportunityCreateV1Evm);

#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug, ToResponse)]
#[serde(tag = "version")]
pub enum OpportunityParamsEvm {
    #[serde(rename = "v1")]
    #[schema(title = "v1")]
    V1(OpportunityParamsV1Evm),
}

#[derive(Serialize, Deserialize, ToSchema, Clone, ToResponse, Debug, PartialEq)]
pub struct OpportunityEvm {
    /// The opportunity unique id.
    #[schema(example = "obo3ee3e-58cc-4372-a567-0e02b2c3d479", value_type = String)]
    pub opportunity_id: OpportunityId,
    /// Creation time of the opportunity (in microseconds since the Unix epoch).
    #[schema(example = 1_700_000_000_000_000i128, value_type = i128)]
    pub creation_time:  UnixTimestampMicros,
    #[serde(flatten)]
    #[schema(inline)]
    pub params:         OpportunityParamsEvm,
}

/// Program specific parameters for the opportunity.
#[serde_as]
#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug, ToResponse)]
#[serde(tag = "program", rename_all = "snake_case")]
#[allow(clippy::large_enum_variant)]
pub enum OpportunityParamsV1ProgramSvm {
    /// Limo program specific parameters for the opportunity.
    /// It contains the Limo order to be executed, encoded in base64.
    /// SDKs will decode this order and create transaction for bidding on the opportunity.
    #[schema(title = "limo")]
    Limo {
        /// The Limo order to be executed, encoded in base64.
        #[schema(example = "UxMUbQAsjrfQUp5stVwMJ6Mucq7VWTvt4ICe69BJ8lVXqwM+0sysV8OqZTdM0W4p...", value_type = String)]
        #[serde_as(as = "Base64")]
        order:         Vec<u8>,
        /// Address of the order account.
        #[schema(example = "DUcTi3rDyS5QEmZ4BNRBejtArmDCWaPYGfN44vBJXKL5", value_type = String)]
        #[serde_as(as = "DisplayFromStr")]
        order_address: Pubkey,
        /// The slot where the opportunity params were fetched from using the RPC.
        #[schema(example = 293106477, value_type = u64)]
        slot:          Slot,
    },
    /// Swap program specific parameters for the opportunity.
    #[schema(title = "swap")]
    Swap {
        /// The user wallet address which requested the quote from the wallet.
        #[schema(example = "DUcTi3rDyS5QEmZ4BNRBejtArmDCWaPYGfN44vBJXKL5", value_type = String)]
        #[serde_as(as = "DisplayFromStr")]
        user_wallet_address: Pubkey,

        /// The user's current balance of the user token
        #[schema(example = 1000)]
        user_mint_user_balance: u64,

        /// The permission account that serves as an identifier for the swap opportunity.
        #[schema(example = "DUcTi3rDyS5QEmZ4BNRBejtArmDCWaPYGfN44vBJXKL5", value_type = String)]
        #[serde_as(as = "DisplayFromStr")]
        permission_account: Pubkey,

        /// The router account to be used for the opportunity execution of the protocol.
        #[schema(example = "DUcTi3rDyS5QEmZ4BNRBejtArmDCWaPYGfN44vBJXKL5", value_type = String)]
        #[serde_as(as = "DisplayFromStr")]
        router_account: Pubkey,

        /// The referral fee in basis points.
        #[schema(example = 10)]
        referral_fee_bps: u16,

        /// The platform fee in basis points.
        #[schema(example = 10)]
        platform_fee_bps: u64,

        /// Specifies whether the fees are to be paid in the searcher or user token.
        #[schema(example = "searcher_token")]
        fee_token: FeeToken,

        /// Details about the tokens to be swapped. Either the searcher token amount or the user token amount must be specified.
        #[schema(inline)]
        tokens: QuoteTokensWithTokenPrograms,

        /// Details about which token accounts need to be initialized an by whom
        #[schema(inline)]
        token_account_initialization_config: TokenAccountInitializationConfig,
    },
}

#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug, ToResponse)]
#[serde(rename_all = "snake_case")]
pub enum FeeToken {
    SearcherToken,
    UserToken,
}

#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug, ToResponse)]
pub enum TokenAccountInitializer {
    Initialized,
    SearcherPayer,
    UserPayer,
}

#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug, ToResponse)]
pub struct TokenAccountInitializationConfig {
    pub user_ata_mint_searcher: TokenAccountInitializer,
    pub user_ata_mint_user: Option<TokenAccountInitializer>, 
    pub router_fee_receiver_ta: TokenAccountInitializer,
    pub relayer_fee_receiver_ata: TokenAccountInitializer,
    pub express_relay_fee_receiver_ata: TokenAccountInitializer,
}


#[serde_as]
#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug, ToResponse)]
#[serde(tag = "side_specified")]
pub enum QuoteTokens {
    #[serde(rename = "searcher")]
    #[schema(title = "searcher_specified")]
    SearcherTokenSpecified {
        /// The token that the searcher will provide
        #[schema(example = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v", value_type = String)]
        #[serde_as(as = "DisplayFromStr")]
        searcher_token:  Pubkey,
        /// The exact amount that the searcher will provide
        searcher_amount: u64,
        /// The token that the user will provide
        #[schema(example = "So11111111111111111111111111111111111111112", value_type = String)]
        #[serde_as(as = "DisplayFromStr")]
        user_token:      Pubkey,
    },
    #[serde(rename = "user")]
    #[schema(title = "user_specified")]
    UserTokenSpecified {
        /// The token that the searcher will provide
        #[schema(example = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v", value_type = String)]
        #[serde_as(as = "DisplayFromStr")]
        searcher_token:             Pubkey,
        /// The token that the user will provide
        #[schema(example = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v", value_type = String)]
        #[serde_as(as = "DisplayFromStr")]
        user_token:                 Pubkey,
        /// The amount that searcher will receive from the user after deducting fees
        user_amount:                u64,
        /// The exact amount that the user will provide, including any fees on the user token side
        user_amount_including_fees: u64,
    },
}

#[serde_as]
#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug, ToResponse)]
pub struct QuoteTokensWithTokenPrograms {
    #[serde(flatten)]
    pub tokens:                 QuoteTokens,
    /// The token program of the searcher mint.
    #[schema(example = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA", value_type = String)]
    #[serde_as(as = "DisplayFromStr")]
    pub token_program_searcher: Pubkey,
    /// The token program of the user mint.
    #[schema(example = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA", value_type = String)]
    #[serde_as(as = "DisplayFromStr")]
    pub token_program_user:     Pubkey,
}


/// Opportunity parameters needed for on-chain execution.
/// Parameters may differ for each program.
#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug, ToResponse)]
pub struct OpportunityParamsV1Svm {
    #[serde(flatten)]
    #[schema(inline)]
    pub program:  OpportunityParamsV1ProgramSvm,
    #[schema(example = "solana", value_type = String)]
    pub chain_id: ChainId,
}

#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug, ToResponse)]
#[serde(tag = "version")]
pub enum OpportunityParamsSvm {
    #[serde(rename = "v1")]
    #[schema(title = "v1")]
    V1(OpportunityParamsV1Svm),
}

#[serde_as]
#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug, ToResponse)]
pub struct OpportunitySvm {
    /// The opportunity unique id.
    #[schema(example = "obo3ee3e-58cc-4372-a567-0e02b2c3d479", value_type = String)]
    pub opportunity_id: OpportunityId,
    /// Creation time of the opportunity (in microseconds since the Unix epoch).
    #[schema(example = 1_700_000_000_000_000i128, value_type = i128)]
    pub creation_time:  UnixTimestampMicros,

    #[serde(flatten)]
    #[schema(inline)]
    pub params: OpportunityParamsSvm,
}

#[derive(Serialize, ToResponse, ToSchema, Clone, Debug, PartialEq)]
#[serde(untagged)]
#[allow(clippy::large_enum_variant)]
pub enum Opportunity {
    Evm(OpportunityEvm),
    Svm(OpportunitySvm),
}

// Default deserialize implementation is not working for opportunity
impl<'de> ::serde::Deserialize<'de> for Opportunity {
    fn deserialize<D>(deserializer: D) -> Result<Opportunity, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        let json_value = serde_json::Value::deserialize(deserializer)?;
        let value: Result<OpportunityEvm, serde_json::Error> =
            serde_json::from_value(json_value.clone());
        match value {
            Ok(opportunity) => Ok(Opportunity::Evm(opportunity)),
            Err(evm_error) => serde_json::from_value(json_value)
                .map(Opportunity::Svm)
                .map_err(|svm_error| {
                    de::Error::custom(format!(
                        "Failed to deserialize opportunity as EVM: {:?}, as SVM: {:?}",
                        evm_error, svm_error
                    ))
                }),
        }
    }
}

fn default_opportunity_mode() -> OpportunityMode {
    OpportunityMode::Live
}
fn default_limit() -> usize {
    20
}

#[derive(Clone, Serialize, Deserialize, IntoParams)]
pub struct GetOpportunitiesQueryParams {
    #[param(example = "op_sepolia", value_type = Option < String >)]
    pub chain_id:       Option<ChainId>,
    /// Get opportunities in live or historical mode.
    #[param(default = "live")]
    #[serde(default = "default_opportunity_mode")]
    pub mode:           OpportunityMode,
    /// The permission key to filter the opportunities by. Used only in historical mode.
    #[param(example = "0xdeadbeef", value_type = Option< String >)]
    pub permission_key: Option<PermissionKeyEvm>,
    /// The time to get the opportunities from.
    #[param(example="2024-05-23T21:26:57.329954Z", value_type = Option<String>)]
    #[serde(default, with = "crate::serde::nullable_datetime")]
    pub from_time:      Option<OffsetDateTime>,
    /// The maximum number of opportunities to return. Capped at 100; if more than 100 requested, at most 100 will be returned.
    #[param(example = "20", value_type = usize, maximum = 100)]
    #[serde(default = "default_limit")]
    pub limit:          usize,
}

#[derive(Serialize, Deserialize, ToSchema, Clone)]
pub struct OpportunityBidEvm {
    /// The opportunity permission key.
    #[schema(example = "0xdeadbeefcafe", value_type=String)]
    pub permission_key: PermissionKeyEvm,
    /// The bid amount in wei.
    #[schema(example = "1000000000000000000", value_type=String)]
    #[serde(with = "crate::serde::u256")]
    pub amount:         U256,
    /// The latest unix timestamp in seconds until which the bid is valid.
    #[schema(example = "1000000000000000000", value_type=String)]
    #[serde(with = "crate::serde::u256")]
    pub deadline:       U256,
    /// The nonce of the bid permit signature.
    #[schema(example = "123", value_type=String)]
    #[serde(with = "crate::serde::u256")]
    pub nonce:          U256,
    /// The executor address.
    #[schema(example = "0x5FbDB2315678afecb367f032d93F642f64180aa2", value_type=String)]
    pub executor:       Address,
    #[schema(
        example = "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef12",
        value_type=String
    )]
    #[serde(with = "crate::serde::signature")]
    pub signature:      Signature,
}

/// Parameters needed to create a new opportunity from the swap request.
#[serde_as]
#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug)]
pub struct QuoteCreateV1SvmParams {
    /// The user wallet address which requested the quote from the wallet. If not provided, an indicative price without a transaction will be returned.
    #[schema(example = "DUcTi3rDyS5QEmZ4BNRBejtArmDCWaPYGfN44vBJXKL5", value_type = Option<String>)]
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub user_wallet_address:    Option<Pubkey>,
    /// The mint address of the token the user will provide in the swap.
    #[schema(example = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v", value_type = String)]
    #[serde_as(as = "DisplayFromStr")]
    pub input_token_mint:       Pubkey,
    /// The mint address of the token the user will receive in the swap.
    #[schema(example = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v", value_type = String)]
    #[serde_as(as = "DisplayFromStr")]
    pub output_token_mint:      Pubkey,
    /// The token amount that the user wants to swap out of/into.
    #[schema(inline)]
    pub specified_token_amount: SpecifiedTokenAmount,
    /// Information about the referral fee and the router to send the fee to. If not provided, referral fee will be set to 0.
    #[schema(inline)]
    pub referral_fee_info:      Option<ReferralFeeInfo>,
    /// The chain id for creating the quote.
    #[schema(example = "solana", value_type = String)]
    pub chain_id:               ChainId,
}

#[serde_as]
#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug)]
pub struct ReferralFeeInfo {
    /// The router account to send referral fees to.
    #[schema(example = "DUcTi3rDyS5QEmZ4BNRBejtArmDCWaPYGfN44vBJXKL5", value_type = String)]
    #[serde_as(as = "DisplayFromStr")]
    pub router:           Pubkey,
    /// The referral fee in basis points.
    #[schema(example = 10, value_type = u16)]
    pub referral_fee_bps: u16,
}

#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug)]
#[serde(tag = "side")]
pub enum SpecifiedTokenAmount {
    #[serde(rename = "input")]
    #[schema(title = "input")]
    UserInputToken {
        #[schema(example = 100)]
        amount: u64,
    },
    #[serde(rename = "output")]
    #[schema(title = "output")]
    UserOutputToken {
        #[schema(example = 50)]
        amount: u64,
    },
}

#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug)]
#[serde(tag = "version")]
pub enum QuoteCreateSvm {
    #[serde(rename = "v1")]
    #[schema(title = "v1")]
    V1(QuoteCreateV1SvmParams),
}

#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug)]
#[serde(untagged)]
pub enum QuoteCreate {
    #[schema(title = "svm")]
    Svm(QuoteCreateSvm),
}

impl QuoteCreate {
    pub fn get_user_wallet_address(&self) -> Option<Pubkey> {
        match self {
            QuoteCreate::Svm(QuoteCreateSvm::V1(params)) => params.user_wallet_address,
        }
    }
}

#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug)]
pub struct QuoteV1Svm {
    /// The transaction for the quote to be executed on chain which is valid until the expiration time. Not provided if the quote to return is only an indicative price.
    #[schema(example = "SGVsbG8sIFdvcmxkIQ==", value_type = Option<String>)]
    #[serde(with = "crate::serde::nullable_transaction_svm")]
    pub transaction:     Option<VersionedTransaction>,
    /// The expiration time of the quote (in seconds since the Unix epoch). Not provided if indicative price.
    #[schema(example = 1_700_000_000_000_000i64, value_type = Option<i64>)]
    pub expiration_time: Option<i64>,
    /// The token and amount that the user needs to send to fulfill the swap transaction.
    pub input_token:     TokenAmountSvm,
    /// The token and amount that the user will receive when the swap is complete.
    pub output_token:    TokenAmountSvm,
    /// The token and amount of the referral fee paid to the party that routed the swap request to Express Relay.
    pub referrer_fee:    TokenAmountSvm,
    /// The token and amount of the platform fee paid to the Express Relay program and relayer.
    pub platform_fee:    TokenAmountSvm,
    /// The chain id for the quote.
    #[schema(example = "solana", value_type = String)]
    pub chain_id:        ChainId,
    /// The reference id for the quote.
    #[schema(example = "beedbeed-58cc-4372-a567-0e02b2c3d479", value_type = String)]
    pub reference_id:    Uuid,
}

#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug)]
#[serde(tag = "version")]
pub enum QuoteSvm {
    #[serde(rename = "v1")]
    #[schema(title = "v1")]
    V1(QuoteV1Svm),
}

#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug)]
#[serde(untagged)]
pub enum Quote {
    #[schema(title = "svm")]
    Svm(QuoteSvm),
}

impl OpportunityCreateSvm {
    pub fn get_program(&self) -> ProgramSvm {
        match self {
            OpportunityCreateSvm::V1(params) => match &params.program_params {
                OpportunityCreateProgramParamsV1Svm::Limo { .. } => ProgramSvm::Limo,
            },
        }
    }
}

// ----- Implementations -----
impl OpportunityEvm {
    pub fn get_chain_id(&self) -> &ChainId {
        match &self.params {
            OpportunityParamsEvm::V1(params) => &params.0.chain_id,
        }
    }
}

impl OpportunitySvm {
    pub fn get_chain_id(&self) -> &ChainId {
        match &self.params {
            OpportunityParamsSvm::V1(params) => &params.chain_id,
        }
    }
}

impl Opportunity {
    pub fn get_chain_id(&self) -> &ChainId {
        match self {
            Opportunity::Evm(opportunity) => opportunity.get_chain_id(),
            Opportunity::Svm(opportunity) => opportunity.get_chain_id(),
        }
    }

    pub fn creation_time(&self) -> UnixTimestampMicros {
        match self {
            Opportunity::Evm(opportunity) => opportunity.creation_time,
            Opportunity::Svm(opportunity) => opportunity.creation_time,
        }
    }
}

impl OpportunityDelete {
    pub fn get_chain_id(&self) -> &ChainId {
        match self {
            OpportunityDelete::Svm(OpportunityDeleteSvm::V1(params)) => &params.chain_id,
            OpportunityDelete::Evm(OpportunityDeleteEvm::V1(params)) => &params.chain_id,
        }
    }
}

#[derive(AsRefStr, Clone)]
#[strum(prefix = "/")]
pub enum Route {
    #[strum(serialize = "")]
    PostOpportunity,
    #[strum(serialize = "quote")]
    PostQuote,
    #[strum(serialize = "")]
    GetOpportunities,
    #[strum(serialize = ":opportunity_id/bids")]
    OpportunityBid,
    #[strum(serialize = "")]
    DeleteOpportunities,
}

impl Routable for Route {
    fn properties(&self) -> crate::RouteProperties {
        let full_path = format!(
            "{}{}{}",
            crate::Route::V1.as_ref(),
            crate::Route::Opportunity.as_ref(),
            self.as_ref()
        )
        .trim_end_matches("/")
        .to_string();
        match self {
            Route::PostOpportunity => crate::RouteProperties {
                access_level: AccessLevel::Public,
                method: http::Method::POST,
                full_path,
            },
            Route::PostQuote => crate::RouteProperties {
                access_level: AccessLevel::Public,
                method: http::Method::POST,
                full_path,
            },
            Route::GetOpportunities => crate::RouteProperties {
                access_level: AccessLevel::Public,
                method: http::Method::GET,
                full_path,
            },
            Route::OpportunityBid => crate::RouteProperties {
                access_level: AccessLevel::Public,
                method: http::Method::POST,
                full_path,
            },
            Route::DeleteOpportunities => crate::RouteProperties {
                access_level: AccessLevel::LoggedIn,
                method: http::Method::DELETE,
                full_path,
            },
        }
    }
}
