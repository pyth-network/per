use ethers::contract::abigen;

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
