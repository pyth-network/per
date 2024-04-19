# Auction Server

The Auction Server is a service that manages the auction process for the ExpressRelay protocol.
It receives bids from searchers, groups them based on the permission key, sorts them, and submits the winning bids to the Express Relay contract using the relayer's private key.

Auctions for each permission key with at least a single bid will be conducted every block (current version has a fixed interval instead of per block):

- Bids will be sorted by a simple heuristic that tries to maximise the profit for the protocols
- The winning bids will be submitted on-chain
- On a successful submission:
  - All the bids involved in the auction will become finalised
- On an unsuccessful submission:
  - Bids will remain in a pending state and will be evaluated again in the next round of auction along with any new bids received in the meantime

⚠️ Auction server will make the best effort to only submit bids that will not revert on-chain but there is no guarantee.
This means that a bid can become public without actual execution

⚠️ Bids can be submitted on-chain multiple times.
The searcher is responsible for implementing security precautions to avoid replay attacks.

⚠️ Any information regarding the bids will remain private until the bid is submitted on-chain.
From that point we consider the winning bids public and publish this information to other searchers too.
Losing bids information will remain private forever.

## Build & Test

This package uses Cargo for building and dependency management.
Simply run `cargo build` and `cargo test` to build and test the project.
We use `sqlx` for database operations, so you need to have a PostgreSQL server running locally.
Check the Migration section for more information on how to setup the database.

Blockchains are configured in `config.yaml`. You can use `config.sample.yaml` as a template.

## Local Development

To start an instance of the webserver for local testing, you first need to perform a few setup steps:

1. Edit `config.yaml` to point to the desired blockchains and Express Relay contracts. You can use `config.sample.yaml` as a template.
2. Generate a secret key to be used for relaying the bids. The Express Relay contract should be deployed with this address as the relayer.

Once you've completed the setup, simply run the following command, using the secret from step (2).

```bash
cargo run -- run --relayer-private-key <relayer-private-key-in-hex-format>
```

This command will start the webservice on `localhost:9000`.

You can check the documentation of the webservice by visiting `localhost:9000/docs`.

## DB & Migrations

sqlx checks the database schema at compile time, so you need to have the database schema up-to-date
before building the project. You can create a `.env` file similar
to the `.env.example` file and set `DATABASE_URL` to the URL of your PostgreSQL database. This file
will be picked up by sqlx-cli and cargo scripts when running the checks.

In the current folder, install sqlx-cli by running `cargo install sqlx-cli`.
Then, run the following command to apply migrations:

```bash
sqlx migrate run
```

We use revertible migrations to manage the database schema. You can create a new migration by running:

```bash
sqlx migrate add -r <migration-name>
```

Since we don't have a running db instance on CI, we use `cargo sqlx prepare` to generate the necessary
info offline. This command will update the `.sqlx` folder.
You need to commit the changes to this folder when adding or changing the queries.
