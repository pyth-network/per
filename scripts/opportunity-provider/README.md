## Overview

This script is designed to provide opportunity using the OpportunityProvider contract. Our objective was to ensure that everything is going to work on mainnet by providing real opportunities for the searchers.

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

Then update the `opportunity.json` and `config.json` files for the opportunity you want to provide, and then run the following command:

```
$ npm run opportunity-provider
```
