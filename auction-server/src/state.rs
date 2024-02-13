use {
    crate::config::{
        ChainId,
        EthereumConfig,
    },
    ethers::{
        providers::{
            Http,
            Provider,
        },
        signers::LocalWallet,
        types::{
            Address,
            Bytes,
            U256,
        },
    },
    serde::{
        Deserialize,
        Serialize,
    },
    std::collections::{
        HashMap,
        HashSet,
    },
    tokio::sync::RwLock,
    utoipa::ToSchema,
    uuid::Uuid,
};

pub type PermissionKey = Bytes;

#[derive(Clone)]
pub struct SimulatedBid {
    pub contract: Address,
    pub calldata: Bytes,
    pub bid:      U256,
    // simulation_time:
}

pub type UnixTimestamp = i64;

#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq)]
pub struct TokenQty {
    /// Token contract address
    #[schema(example = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",value_type=String)]
    pub contract: ethers::abi::Address,
    /// Token amount
    #[schema(example = "1000", value_type=String)]
    #[serde(with = "crate::serde::u256")]
    pub amount:   U256,
}

/// Opportunity parameters needed for on-chain execution
/// If a searcher signs the opportunity and have approved enough tokens to liquidation adapter,
/// by calling this contract with the given calldata and structures, they will receive the tokens specified
/// in the receipt_tokens field, and will send the tokens specified in the repay_tokens field.
#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq)]
pub struct OpportunityParamsV1 {
    /// The permission key required for succesful execution of the liquidation.
    #[schema(example = "0xdeadbeefcafe", value_type=String)]
    pub permission_key: Bytes,
    /// The chain id where the liquidation will be executed.
    #[schema(example = "sepolia", value_type=String)]
    pub chain_id:       ChainId,
    /// The contract address to call for execution of the liquidation.
    #[schema(example = "0xcA11bde05977b3631167028862bE2a173976CA11", value_type=String)]
    pub contract:       ethers::abi::Address,
    /// Calldata for the contract call.
    #[schema(example = "0xdeadbeef", value_type=String)]
    pub calldata:       Bytes,
    /// The value to send with the contract call.
    #[schema(example = "1", value_type=String)]
    #[serde(with = "crate::serde::u256")]
    pub value:          U256,

    pub repay_tokens:   Vec<TokenQty>,
    pub receipt_tokens: Vec<TokenQty>,
}

#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq)]
#[serde(tag = "version")]
pub enum OpportunityParams {
    #[serde(rename = "v1")]
    V1(OpportunityParamsV1),
}

#[derive(Clone, PartialEq)]
pub struct LiquidationOpportunity {
    pub id:            Uuid,
    pub creation_time: UnixTimestamp,
    pub params:        OpportunityParams,
    pub bidders:       HashSet<Address>,
}

#[derive(Clone)]
pub enum SpoofInfo {
    Spoofed {
        balance_slot:   U256,
        allowance_slot: U256,
    },
    UnableToSpoof,
}

pub struct ChainStore {
    pub provider:         Provider<Http>,
    pub network_id:       u64,
    pub config:           EthereumConfig,
    pub token_spoof_info: RwLock<HashMap<Address, SpoofInfo>>,
    pub bids:             RwLock<HashMap<PermissionKey, Vec<SimulatedBid>>>,
}

#[derive(Default)]
pub struct LiquidationStore {
    pub opportunities: RwLock<HashMap<PermissionKey, Vec<LiquidationOpportunity>>>,
}

pub struct Store {
    pub chains:            HashMap<ChainId, ChainStore>,
    pub liquidation_store: LiquidationStore,
    pub per_operator:      LocalWallet,
}
