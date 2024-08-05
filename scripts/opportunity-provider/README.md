## Overview

This script is designed to provide opportunity using the OpportunityProvider contract.

## Installation

### npm

```
$ npm install
```

## Quickstart

### Approve Tokens

To provide a new opportunity, first make sure that you approved your tokens for `Permit2` contract using the command below:

```
# Approve Permit2 to use them
cast send \
--private-key $PRIVATE_KEY \
--rpc-url $RPC_URL  \
$TOKEN \
"approve(address spender, uint256 value)" \
$PERMIT2 $AMOUNT
```

### Provide Opportunity

Then update the `opportunities.json` and `config.json` files for the opportunity you want to provide, and then run the following command:

```
$ npm run provide-opportunity -- --private-key $PRIVATE_KEY
```

To create and submit random opportunities for load test, first update the `tokens.json` file, and use the following command:

```
$ npm run provide-opportunity -- --private-key $PRIVATE_KEY --load-test --count 10
```
