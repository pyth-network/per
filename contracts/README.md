# Contracts

## Overview

[Express Relay](./ExressRelay.md)
[Opportunity Adapter](./OpportunityAdapter.md)

## Setup

Run the following commands to install necessary libraries:

```shell
$ npm install
$ forge install foundry-rs/forge-std@v1.8.0 --no-git --no-commit
$ forge install OpenZeppelin/openzeppelin-contracts@v5.0.2 --no-git --no-commit
$ forge install OpenZeppelin/openzeppelin-contracts-upgradeable@v4.9.6 --no-git --no-commit
$ forge install Uniswap/permit2@0x000000000022D473030F116dDEE9F6B43aC78BA3 --no-git --no-commit
$ forge install nomad-xyz/ExcessivelySafeCall@be417ab0c26233578b8d8f3a37b87bd1fcb4e286 --no-git --no-commit
```

## Permit2

Since Permit2 contract compiles with a different solc version not compatible with the rest of the repo,
we have commited the compiled artifacts in [`permit2/Permit2.json`](`permit2/Permit2.json`).
To compile the contract yourself, you can run the following command:

```shell
FOUNDRY_PROFILE=permit2 forge build --via-ir
```

## Repo contracts

**The contracts included in `src/` includes a protocol around which we test liquidation calls via the express relay contract.** The protocol is a sample token vault where anyone can permissionlessly create a vault with collateral and debt positions, somewhat like (though simpler than) a vanilla lending protocol. This protocol is found in `TokenVault.sol`, and its associated searcher contract can be found in `SearcherVault.sol`. This protocol uses the mock Pyth contract found in the Solidity SDK.

The Express Relay main contract is in `ExpressRelay.sol`.
It includes functionality to call into arbitrary contracts with arbitrary calldata via an external `call` (as opposed to `delegatecall`,
since we need to alter the state of the end protocol that we call into).
We also have the opportunity adapter contract in `OpportunityAdapter.sol`, which calls into arbitrary protocols' liquidation contracts along with checks that the tokens spent and received by the end user meet expectations.
This allows users to participate in liquidations without needing to set up their own searcher contracts and do bespoke integration work.

Tests can be found in `test/`. These tests include checks that the protocol functions work, as well as checks around permissioning, bid conditions, and appropriate failing of components of the express relay bundle (without failing the whole bundle).

To run tests with the appropriate stack depth and console logging, run

```shell
$ forge test -vvv --via-ir
```

You can also run a local validator via `anvil --gas-limit 500000000000000000 --block-time 2`, changing the values for the gas limit and block time as desired. Note that if you omit the `--block-time` flag, the local network will create a new block for each transaction (similar to how Optimism created L2 blocks pre-Bedrock). Running `auction_offchain.py` will spit out the final call to `forge script` you should run to send the transaction to the localnet.

To run the script runs in `Vault.s.sol`, you should startup the local validator and create a `.env` file with the `PRIVATE_KEY` env variable which is used for submitting the transactions. For localnet, the private key saved should correspond to an address that has a bunch of ETH seeded by Forge, essentially one of the mnemonic wallets when you start up anvil. Then, run the necessary setup commands:

1. Set up contracts and save to an environment JSON.

```shell
$ forge script script/Vault.s.sol --via-ir --fork-url http://localhost:8545 --private-key 0xf46ea803192f16ef1c4f1d5fb0d6060535dbd571ea1afc7db6816f28961ba78a -vvv --sig 'setUpLocalnet()' --broadcast
```

2. Set oracle prices to allow for vault creation.

```shell
$ forge script script/Vault.s.sol --via-ir --fork-url http://localhost:8545 --private-key 0xf46ea803192f16ef1c4f1d5fb0d6060535dbd571ea1afc7db6816f28961ba78a -vvv --sig 'setOraclePrice(int64,int64,uint64)' 110 110 190 --broadcast
```

3. Vault creation.

```shell
$ forge script script/Vault.s.sol --via-ir --fork-url http://localhost:8545 --private-key 0xf46ea803192f16ef1c4f1d5fb0d6060535dbd571ea1afc7db6816f28961ba78a -vvv --sig 'setUpVault(uint256,uint256,bool)' 100 80 true --broadcast
```

4. Undercollateralize the vault by moving prices.

```shell
$ forge script script/Vault.s.sol --via-ir --fork-url http://localhost:8545 --private-key 0xf46ea803192f16ef1c4f1d5fb0d6060535dbd571ea1afc7db6816f28961ba78a -vvv --sig 'setOraclePrice(int64,int64,uint64)' 110 200 200 --broadcast
```

5. Submit the PER bundle. Run the command spit out by the auction script. Because of the call to `vm.roll`, this essentially does a simulate and can be run repeatedly from this state.

Note that the `--private-key` flag is necessary in order to run some of the commands above; this is because Forge requires specification of a default sender wallet from which the transactions are sent.

In order to enable forge to write to the filesystem (which is needed in order to save some of the variables in the steps above), please navigate to `foundry.toml` and add the following line if it does not already exist:

```
fs_permissions = [{ access = "read-write", path = "./"}]
```

This permits the vm to access any file in the root directory via read-write operations.

# Verification

For verifying contracts, you can use the `forge verify-contract` command.
For example, to verify the ERC1967Proxy contract on the Optimism network, you can run the following commands:

```
forge verify-contract --via-ir <contract-address> ERC1967Proxy --verifier blockscout --verifier-url https://optimism-sepolia.blockscout.com/api/ --chain-id 11155420
forge verify-contract --via-ir <contract-address> ERC1967Proxy --verifier-url https://api-sepolia-optimistic.etherscan.io/api --etherscan-api-key <optimistic-etherscan-api-key> --chain-id 11155420

You may have to specify the constructor arguments used to initialize the contract, using the `--constructor-args` flag. For more info see the [forge instructions on verifying contracts](https://book.getfoundry.sh/forge/deploying?highlight=verify#verifying-a-pre-existing-contract).
```

# Upgrading contracts on optimism testnet

Run the following to upgrade ExpressRelay and OpportunityAdapter contracts on the optimism testnet:

```
forge script script/Vault.s.sol --via-ir --fork-url https://sepolia.optimism.io -vvv --sig 'upgradeExpressRelay(address)' 0xc643e55EE8944F3017F4CB8C82aa3DB1AA2d8941 --broadcast
forge script script/Vault.s.sol --via-ir --fork-url https://sepolia.optimism.io -vvv --sig 'upgradeOpportunityAdapter(address)' 0xD35E56C06320B1ED549a8F85d316FEc854FF4b71 --broadcast
```
