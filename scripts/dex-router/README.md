## Overview

This script bids to fulfill Limo orders via routing through on-chain liquidity.

## Installation

### npm

```
$ npm install
```

## Quickstart

### Funding

This script routes opportunities via on-chain liquidity, so your wallet will not need to hold any SPL tokens. However, it will need to hold some SOL to pay the gas cost.

### Create Lookup Table

Often, the on-chain liquidity routing instructions invoke a large number of accounts. As a result, the resulting set of instructions may be too large to fit into a single transaction. To minimize the size of the transaction, you can create a lookup table to store accounts commonly invoked in instructions like the Express Relay `SubmitBid` instruction using the [`solana address-lookup-table` CLI](https://docs.solanalabs.com/cli/usage#solana-address-lookup-table).

### Run DEX Router

To run the DEX router, you can run the following command:

```
npm run dex-router -- \
--sk-executor $PRIVATE_KEY \
--chain-id development-solana \
--endpoint-express-relay https://per-staging.dourolabs.app/ \
--endpoint-svm https://api.mainnet-beta.solana.com
```

This command will subscribe to Limo opportunities with the `development-solana` chain ID and create 0-SOL bids to submit to the auction server that route the order through on-chain liquidity using the `FlashTakeOrder` functionality of the Limo program.
