# Express Relay

## [Off-chain server](auction-server/README.md)

## [Contracts](contracts/README.md)

## SDKs

You can use the following SDKs to integrate with the Express Relay APIs and contracts:

- [Javascript SDK](https://github.com/pyth-network/pyth-crosschain/tree/main/express_relay/sdk/js)
- [Python SDK](https://github.com/pyth-network/pyth-crosschain/tree/main/express_relay/sdk/python)
- [Solidity SDK](https://github.com/pyth-network/pyth-crosschain/tree/main/express_relay/sdk/solidity)

### pre-commit hooks

pre-commit is a tool that checks and fixes simple issues (formatting, ...) before each commit.
You can install it by following [their website](https://pre-commit.com/).
In order to enable checks for this repo run `pre-commit install` from command-line in the root of this repo.

The checks are also performed in the CI to ensure the code follows consistent formatting.

### Development with Tilt

Since express relay is a multi-service project, we use [Tilt](https://tilt.dev/) to manage the development environment.
It is a great tool for local development and testing.
Tilt requires `anvil`, `forge`, `poetry`, rust, and the Solana CLI to be installed on your machine.

Here are the installation instructions for each:

- Rust: https://www.rust-lang.org/tools/install
- Foundry (anvil,forge,cast, etc.): https://book.getfoundry.sh/getting-started/installation
- Poetry: https://python-poetry.org/docs/#installation
- Tilt: https://docs.tilt.dev/install.html
- Solana CLI: https://docs.solanalabs.com/cli/install

Note that for the Solana CLI, you may need to alter your terminal's `PATH` variable to include the Solana programs. To make this work with Tilt, you should include the `PATH` update in your `~/.bashrc` or `~/.zshrc` file depending on which shell your machine uses.

Run `tilt up` in the root of the repo to start the development environment.
You can access the ui at `http://localhost:10350/`.

Here is what Tilt up does in order:

1. [EVM] Starts `anvil`: local EVM chain to test the contracts with
2. [EVM] Deploy express relay contracts
3. [SVM] Builds SVM programs
4. [SVM] Starts `solana-test-validator`: Solana localnet to test the programs with
5. [SVM] Airdrops SOL to searcher, admin, and relayer signer wallet
6. [SVM] Initializes the SVM programs on the localnet
7. Start the auction server
8. [EVM] Start the liquidation monitor
9. [EVM] Start the simple searcher
10. [SVM] Submits an SVM transaction bid to the auction server

There are some useful gadgets in Tilt ui for creating new vaults and checking the vault status.
You can use them to test the system end to end.

You can modify the services and restart the resources as necessary.

## License

The primary license for source codes available in this repo is the Apache 2.0 (`Apache-2.0`), see [LICENSE](./LICENSE). Minus the following exceptions:

- [Express Relay Auction Server](./auction-server) has a `BUSL-1.1` license
