# Auction Server

A single instance of this webservice can simultaneously serve random numbers for several different blockchains.
Each blockchain is configured in `config.yaml`.

## Build & Test

This package uses Cargo for building and dependency management.
Simply run `cargo build` and `cargo test` to build and test the project.

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

## Migrations

Install sqlx-cli by running `cargo install sqlx-cli`. Then, run the following command to apply migrations:

```bash
export DATABASE_URL=postgres://postgres@localhost/postgres
sqlx migrate run
```

We use revertible migrations to manage the database schema. You can create a new migration by running:

```bash
sqlx migrate add -r <migration-name>
```
