# Express Relay

## [Off-chain server](auction-server/README.md)

## [Contracts](per_multicall/README.md)

### pre-commit hooks

pre-commit is a tool that checks and fixes simple issues (formatting, ...) before each commit.
You can install it by following [their website](https://pre-commit.com/).
In order to enable checks for this repo run `pre-commit install` from command-line in the root of this repo.

The checks are also performed in the CI to ensure the code follows consistent formatting.

### Development with Tilt

Since express relay is a multi-service project, we use [Tilt](https://tilt.dev/) to manage the development environment.
It is a great tool for local development and testing.
Tilt requires `anvil`, `forge`, `poetry`, and rust to be installed on your machine.

Run `tilt up` in the root of the repo to start the development environment.
You can access the ui at `http://localhost:10350/`.

Here is what tilt up does in order:

1. Starts `anvil`: local EVM chain to test the contracts with
2. Deploy express relay contracts
3. Start the auction server
4. Start the liquidation monitor
5. Start the simple searcher

There are some useful gadgets in Tilt ui for creating new vaults and checking the vault status.
You can use them to test the system end to end.

You can modify the services and restart the resources as necessary.
