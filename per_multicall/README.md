# PER

## Setup

Run the following commands to install necessary libraries:

```shell
$ npm install
$ forge install foundry-rs/forge-std --no-git --no-commit
$ forge install OpenZeppelin/openzeppelin-contracts --no-git --no-commit
```

## Repo contracts

**The contracts included in `src/` includes a protocol around which we test fast calls via the multicall contract.** The protocol is a sample token vault where anyone can permissionlessly create a vault with collateral and debt positions, somewhat like (though simpler than) a vanilla lending protocol. This protocol is found in `TokenVault.sol`, and its associated searcher contract can be found in `SearcherVault.sol`. This protocol uses the mock Pyth contract found in the Solidity SDK.

The multicall contract is in `PERMulticall.sol`. It includes functionality to call into arbitrary contracts with arbitrary calldata via an external `call` (as opposed to `delegatecall`, since we need to alter the state of the end protocol that we call into). We also have the liquidation adapter contract in `LiquidationAdapter.sol`, which calls into arbitrary protocols' liquidation contracts along with checks that the tokens spent and received by the end user meet expectations. This allows users to participate in liquidations without needing to set up their own searcher contracts and do bespoke integration work.

Tests can be found in `test/`. These tests include checks that the protocol functions work, as well as checks around permissioning, bid conditions, and appropriate failing of components of the multicall bundle (without failing the whole bundle).

To run tests with the appropriate stack depth and console logging, run

```shell
$ forge test -vvv --via-ir
```

You can also run a local validator via `anvil --gas-limit 500000000000000000 --block-time 2`, changing the values for the gas limit and block time as desired. Note that if you omit the `--block-time` flag, the local network will create a new block for each transaction (similar to how Optimism created L2 blocks pre-Bedrock). Running `auction_offchain.py` will spit out the final call to `forge script` you should run to send the transaction to the localnet.

To run the script runs in `Vault.s.sol`, you should startup the local validator and create a `.env` file with the `PRIVATE_KEY` env variable which is used for submitting the transactions. Then, run the necessary setup commands:

1. Set up contracts and save to an environment JSON.

```shell
$ forge script script/Vault.s.sol --via-ir --fork-url http://localhost:8545 --sender 0xd6e417287b875a3932c1ff5dcb26d4d2c8b90b40 -vvv --sig 'setUpContracts()' --broadcast
```

2. Set oracle prices to allow for vault creation.

```shell
$ forge script script/Vault.s.sol --via-ir --fork-url http://localhost:8545 --sender 0xd6e417287b875a3932c1ff5dcb26d4d2c8b90b40 -vvv --sig 'setOraclePrice(int64,int64,uint64)' 110 110 190 --broadcast
```

3. Vault creation.

```shell
$ forge script script/Vault.s.sol --via-ir --fork-url http://localhost:8545 --sender 0xd6e417287b875a3932c1ff5dcb26d4d2c8b90b40 -vvv --sig 'setUpVault(uint256,uint256,bool)' 100 80 true --broadcast
```

4. Undercollateralize the vault by moving prices.

```shell
$ forge script script/Vault.s.sol --via-ir --fork-url http://localhost:8545 --sender 0xd6e417287b875a3932c1ff5dcb26d4d2c8b90b40 --private-key 0xf46ea803192f16ef1c4f1d5fb0d6060535dbd571ea1afc7db6816f28961ba78a -vvv --sig 'setOraclePrice(int64,int64,uint64)' 110 200 200 --broadcast
```

5. Submit the PER bundle. Run the command spit out by the auction script. Because of the call to `vm.roll`, this essentially does a simulate and can be run repeatedly from this state.

In order to enable forge to write to the filesystem (which is needed in order to save some of the variables in the steps above), please navigate to `foundry.toml` and add the following line if it does not already exist:

```
fs_permissions = [{ access = "read-write", path = "./"}]
```

This permits the vm to access any file in the root directory via read-write operations.
