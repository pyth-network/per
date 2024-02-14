# PER

## Off-chain server

Run `uvicorn main:app --reload` to run the FastAPI server. This enables the endpoint for submitting and reading searcher submissions.

## Off-chain auction

In order to run the off-chain auction mechanism, run `python3 -m auction_offchain`. You can run `searcher.py` to submit some prescripted bids to the offchain auction pool, and running `auction_offchain.py` will run the determination for the auction, culminating in a bundle of calls to submit to the multicall contract.

This bundle is automatically saved in an environment file at `.env`. In order to save these variables into your environment, you should run the following commands to source the newly created `.env` file:

```shell
$ set -a
$ source .env
$ set +a
```

The updated enviornment variables can then be seen via `env`. We can then run the appropriate forge tests which will pull the relevant bundle information from the environment variables. To do this, run `forge test -vvv --via-ir --match-test {TestToBeRun}`. Note that you need to `source` the `.env` file in the same session as the one in which you run the forge tests.

### pre-commit hooks

pre-commit is a tool that checks and fixes simple issues (formatting, ...) before each commit. You can install it by following [their website](https://pre-commit.com/). In order to enable checks for this repo run `pre-commit install` from command-line in the root of this repo.

The checks are also performed in the CI to ensure the code follows consistent formatting.

### Development with Tilt

Run `tilt up --namespace dev-<YOUR_NAMESPACE>` to start tilt.

## Testing

You can run forge tests from `per_multicall/` with the `--via-ir` flag. This only tests the smart contracts and can be used to evaluate whether any changes to the smart contracts preserve the desired behavior specified by the tests.

To run a happy path test of the on-chain contracts plus the off-chain services, follow the following steps. You will need a valid EVM private key saved as SK_TX_SENDER to submit forge transactions from.

1. Run `anvil --gas-limit 500000000000000000 --block-time 2`. Retrieve the localhost url and save as ANVIL_RPC_URL.
2. Run `forge script script/Vault.s.sol --via-ir --fork-url ${ANVIL_RPC_URL} --private-key ${SK_TX_SENDER} -vvv --sig 'setUpHappyPath()' --broadcast` from `per_multicall/`.
3. Run `forge script script/Vault.s.sol --via-ir --fork-url ${ANVIL_RPC_URL} --private-key ${SK_TX_SENDER} -vvv --sig 'getVault(uint256)' 0 --broadcast` from `per_multicall/`. Confirm that the logged vault amounts are nonzero.
4. Retrieve the following information from `per_multicall/latestEnvironment.json`:
   a. Retrieve the address saved under "multicall" and save as MULTICALL.
   b. Retrieve the address saved under "liquidationAdapter" and save as ADAPTER.
   c. Retrieve the address saved under "tokenVault" and save as TOKEN_VAULT.
   d. Retrieve the address saved under "weth" and save as WETH.
   e. Retreive the number saved under "perOperatorSk", convert to a hex string, and save as OPERATOR_SK. You can perform this conversion in Python by calling hex() on the number.
   f. Retrieve the number saved under "searcherAOwnerSk", convert to a hex string, and save as SEARCHER_SK. You can perform this conversion in Python by calling hex on the number.
5. Create a file `auction-server/config.yaml`. Follow the format in the template `auction-server/config.sample.yaml`. Under the chain `development`, set
   a. `geth_rpc_addr` to the value stored in ANVIL_RPC_URL
   b. `per_contract` to the value stored in MULTICALL
   c. `adapter_contract` to the value stored in ADAPTER
6. Run `cargo run -- run --per-private-key ${OPERATOR_SK}` from `auction-server/`. This should start up the auction server.
7. Run `python3 -m per_sdk.protocols.token_vault_monitor --chain-id development --rpc-url ${ANVIL_RPC_URL} --vault-contract ${TOKEN_VAULT} --weth-contract ${WETH} --liquidation-server-url http://localhost:9000/liquidation/submit_opportunity --mock-pyth`. This should start up the monitor script that exposes liquidatable vaults to the liquidation monitor server.
8. Run `python3 -m per_sdk.searcher.simple_searcher --private-key ${SEARCHER_SK} --chain-id development --verbose --liquidation-server-url http://localhost:9000`.
9. Run `forge script script/Vault.s.sol --via-ir --fork-url ${ANVIL_RPC_URL} --private-key ${SK_TX_SENDER} -vvv --sig 'getVault(uint256)' 0 --broadcast` from `per_multicall/`. Confirm that the logged vault amounts are now 0--this indicates that the vault was properly liquidated.
