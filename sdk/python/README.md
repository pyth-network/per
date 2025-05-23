# Express Relay Python SDK

Utility library for searchers and protocols to interact with the Express Relay API.

The SDK includes searcher-side utilities and protocol-side utilities. The searcher-side utilities include a basic Searcher client for connecting to the Express Relay server as well as an example SimpleSearcher class that provides a simple workflow for assessing and bidding on liquidation opportunities.

## Installation

### poetry

```
$ poetry add express-relay
```

## Development

You can use `anchorpy` to generate a Python client of the Express Relay program.

`anchorpy` only works with the old Anchor IDL standard. You will need to use `anchor` `0.29.0`: make sure you modify `anchor_version` in `Anchor.toml` to `0.29.0` and run `anchor idl parse --file` to generate the IDL.

You can generate the Python client from the IDL via:

```bash
poetry run anchorpy client-gen express_relay/idl/idlExpressRelay.json express_relay/svm/generated/express_relay --program-id PytERJFhAKuNNuaiXkApLfWzwNwSNDACpigT3LwQfou
```

## Quickstart

To run the simple searcher script, navigate to `python/` and run the following command:

### Evm

```
$ poetry run python3 -m express_relay.searcher.examples.simple_searcher_evm \
--private-key <PRIVATE_KEY_HEX_STRING> \
--chain-id development \
--verbose \
--server-url https://per-staging.dourolabs.app/
```

This simple example runs a searcher that queries the Express Relay liquidation server for available liquidation
opportunities and naively submits a bid on each available opportunity.

### Svm

```
$ poetry run python3 -m express_relay.searcher.examples.simple_searcher_svm \
--endpoint-express-relay https://per-staging.dourolabs.app/ \
--chain-id development-solana \
--private-key-json-file <PATH_TO_JSON_FILE> \
--endpoint-svm https://api.mainnet-beta.solana.com \
--bid 10000000 # Bid amount in lamports
```
