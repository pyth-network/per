## Overview

This script provides opportunities around the Limo prgoram by creating limit orders for searchers to fulfill.

## Installation

### npm

```
$ npm install
```

## Quickstart

### Funding

To provide a new opportunity, you must first fund your wallet with the assets you wish to create opportunities with.

### Provide Opportunity

Then create an `opportunities.json` file for the opportunities you want to provide. You can see a sample list of opportunities in `opportunities.sample.json`:

```
[
  {
    "token1": {
      "mint": "So11111111111111111111111111111111111111112",
      "symbol": "SOL"
    },
    "token2": {
      "mint": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
      "symbol": "USDC"
    },
    "randomizeSides": true,
    "minAmountNotional": 0.01,
    "maxAmountNotional": 0.1
  }
]
```

You can include multiple opportunity pairs in this list. For a given opportunity type, if you set the `randomizeSides` option to `true`, then each opportunity created will randomly set token1 as either the input or output token. If `randomizeSides` is set to `false`, then token1 will be the input token (your wallet sells this token) and token2 will be the output token (your wallet will receive this token upon order fulfillment). The notional amounts in `minAmountNotional` and `maxAmountNotional` are in USD; if you wish to use a fixed notional amount each time, you can set `minAmountNotional` equal to `maxAmountNotional`.

After configuring your opportunities JSON, you can run the following command to start the script that will create opportunities in a loop:

```
$ npm run create-order -- \
--sk-payer $PRIVATE_KEY \
--global-config $GLOBAL_CONFIG \
--opportunities $OPPORTUNITIES_FILEPATH \
--endpoint-svm $RPC_URL \
--markup 100 \
--interval 900
```

The `markup` argument defines in basis points the markup of the input token assets being sold relative to the output token assets being bought, based on market prices. The `interval` argument defines how many seconds the script will wait between creating opportunities; if you wish to run the script once as opposed to in a loop, do not provide the `interval` argument.
