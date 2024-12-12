# Limonade

> When solana gives you limo, make limonade -- Unknown

This script is used to submit new opportunities fetched from the limo program to the express relay.

How to run:

```bash
npm run limonade -- \
--global-config TeStcUQMmECYEtdeXo9cXpktQWaGe4bhJ7zxAUMzB2X \
--endpoint https://per-staging.dourolabs.app/ \
--chain-id development-solana \
--api-key <API_KEY_FOR_LIMO_PROFILE> \
--rpc-endpoint <RPC_URL>
```

## Using Hermes prices

Limonade can use Hermes prices to only submit opportunities that are likely to be executed against (because their implied price is close to the current market price of the pair). This is done with the optional `--price-config` argument that takes in the path to a `price-config.yaml` with the assets whose prices you want subscribe to.

Please see `price-config.sample.yaml` for a sample config. The `id` field corresponds to the [Pyth Price Feed Id](https://www.pyth.network/developers/price-feed-ids) of the asset and the `mint` is simply the SPL mint of the token.
