use {
    super::traced_client::TracedClient,
    ethers::{
        contract::abigen,
        middleware::{
            gas_oracle::GasOracleMiddleware,
            transformer::{
                Transformer,
                TransformerError,
            },
            NonceManagerMiddleware,
            SignerMiddleware,
            TransformerMiddleware,
        },
        providers::{
            Middleware,
            Provider,
        },
        signers::{
            LocalWallet,
            Signer,
        },
        types::{
            transaction::eip2718::TypedTransaction,
            Address,
            Bytes,
            TransactionRequest,
            H160,
            U256,
        },
    },
    gas_oracle::EthProviderOracle,
};

abigen!(
    OpportunityAdapter,
    "../contracts/evm/out/OpportunityAdapter.sol/OpportunityAdapter.json";
    AdapterFactory,
    "../contracts/evm/out/OpportunityAdapterFactory.sol/OpportunityAdapterFactory.json"
);
abigen!(ERC20, "../contracts/evm/out/ERC20.sol/ERC20.json");
abigen!(WETH9, "../contracts/evm/out/WETH9.sol/WETH9.json");

abigen!(
    ExpressRelay,
    "../contracts/evm/out/ExpressRelay.sol/ExpressRelay.json"
);

impl From<([u8; 16], H160, Bytes, U256, U256, bool)> for MulticallData {
    fn from(x: ([u8; 16], H160, Bytes, U256, U256, bool)) -> Self {
        MulticallData {
            bid_id:            x.0,
            target_contract:   x.1,
            target_calldata:   x.2,
            bid_amount:        x.3,
            gas_limit:         x.4,
            revert_on_failure: x.5,
        }
    }
}

/// Transformer that converts a transaction into a legacy transaction if use_legacy_tx is true.
#[derive(Clone, Debug)]
pub struct LegacyTxTransformer {
    pub use_legacy_tx: bool,
}

impl Transformer for LegacyTxTransformer {
    fn transform(&self, tx: &mut TypedTransaction) -> Result<(), TransformerError> {
        if self.use_legacy_tx {
            let legacy_request: TransactionRequest = (*tx).clone().into();
            *tx = legacy_request.into();
            Ok(())
        } else {
            Ok(())
        }
    }
}

pub type ExpressRelayContractEvm = ExpressRelay<Provider<TracedClient>>;
pub type SignableProvider = TransformerMiddleware<
    GasOracleMiddleware<
        NonceManagerMiddleware<SignerMiddleware<Provider<TracedClient>, LocalWallet>>,
        EthProviderOracle<Provider<TracedClient>>,
    >,
    LegacyTxTransformer,
>;
pub type SignableExpressRelayContract = ExpressRelay<SignableProvider>;

impl SignableExpressRelayContract {
    pub fn get_relayer_address(&self) -> Address {
        self.client().inner().inner().inner().signer().address()
    }
}
