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

Often, the on-chain liquidity routing instructions invoke a large number of accounts. As a result, the resulting set of instructions may be too large to fit into a single transaction. To minimize the size of the transaction, you can use a lookup table to store accounts commonly invoked in instructions like the Express Relay `SubmitBid` instruction. You can use the utility methods in `utils/lookupTable.ts` to create a lookup table and add public keys to it via the following command:

```
npm run lookup-table -- \
--sk-auth $PRIVATE_KEY \
--endpoint-svm https://api.mainnet-beta.solana.com \
--addresses-to-add <LIST_OF_PUBKEYS>
--create-lookup-table
```

Make sure to add the address of the created lookup table to `LOOKUP_TABLE_ADDRESSES` in `const.ts` for your relevant `chainId`.

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
