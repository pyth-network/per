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
    std::collections::HashMap,
    tokio::sync::RwLock,
    utoipa::ToSchema,
};

pub type PermissionKey = Bytes;
pub type Contract = Address;


#[derive(Serialize, Deserialize, ToSchema, Clone)]
pub struct Opportunity {
    /// The chain id to bid on.
    #[schema(example = "sepolia")]
    pub chain_id:   String,
    /// The contract address to call.
    #[schema(example = "0xcA11bde05977b3631167028862bE2a173976CA11")]
    pub contract:   String,
    /// Calldata for the contract call.
    #[schema(example = "0xdeadbeef")]
    calldata:       String,
    /// The permission key to bid on.
    #[schema(example = "0xdeadbeef")]
    permission_key: String,
    /// The ID of the account/vault that is eligible for liquidation
    #[schema(example = "4")]
    account:        String,
    /// A list of repay tokens with amount
    // #[schema(example = vec![("0x6B175474E89094C44Da98b954EedeAC495271d0F", 1_000_000)])]
    repay_tokens: Vec<(String, U256)>,
    /// A list of receipt tokens with amount
    // #[schema(example = vec![("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2", 500)])]
    receipt_tokens: Vec<(String, U256)>,
    /// A list of prices in the format of PriceFeed
    // #[schema(example = [("0xdeadbeef", (100, 2, 0, 1_700_000_000), (101, 1, 0, 1_700_000_000), "0xdeadbeef")])]
    prices: Vec<(
        String,
        (U256, U256, U256, U256),
        (U256, U256, U256, U256),
        String,
    )>,
}

#[derive(Deserialize)]
pub struct GetOppsParams {
    pub chain_id: String,
    pub contract: Option<String>,
}

impl Default for GetOppsParams {
    fn default() -> Self {
        Self {
            chain_id: "development".to_string(),
            contract: None,
        }
    }
}

#[derive(Clone)]
pub struct SimulatedBid {
    pub contract: Address,
    pub calldata: Bytes,
    pub bid:      U256,
    // simulation_time:
}

pub struct ChainStore {
    pub provider:   Provider<Http>,
    pub network_id: u64,
    pub config:     EthereumConfig,
    pub bids:       RwLock<HashMap<PermissionKey, Vec<SimulatedBid>>>,
    pub opps:       RwLock<HashMap<Contract, Vec<Opportunity>>>,
}


pub struct Store {
    pub chains:       HashMap<ChainId, ChainStore>,
    pub per_operator: LocalWallet,
}
