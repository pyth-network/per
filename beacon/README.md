# Beacon

The Beacon is the off-chain service that exposes liquidation opportunities on integrated protocols to searchers. Protocols surface their liquidatable vaults/accounts along with the calldata and the token denominations and amounts involved in the transaction. Searchers can query these opportunities from the Beacon server. If they wish to act on an opportunity, they can simply construct a transaction based off the information in the opportunity.

The LiquidationAdapter contract that is part of the Express Relay on-chain stack allows searchers to perform liquidations across different protocols without needing to deploy their own contracts or perform bespoke engineering work. The Beacon service is important in enabling this, as it disseminates the calldata that searchers need to include in the transactions they construct.

Each protocol that integrates with Express Relay and the LiquidationAdapter workflow must provide code that handles getting liquidatable accounts; the template and example files for this are found in `/protocols`. Some common types are defined in `utils/types_liquidation_adapter.py`, and standard functions for accessing Pyth Hermes prices can be found in `utils/pyth_prices.py`.

The party that runs the beacon can run the protocol-provided file to get and surface liquidatable accounts to the Beacon server.
