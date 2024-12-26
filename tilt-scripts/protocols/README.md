# Opportunity Monitor

The monitor is the off-chain service that exposes liquidation opportunities on integrated protocols to searchers. Protocols surface their liquidatable vaults/accounts along with the calldata and the token denominations and amounts involved in the transaction. Searchers can query these opportunities from the liquidation server. If they wish to act on an opportunity, they can simply construct a signature based off the information in the opportunity.

The LiquidationAdapter contract that is part of the Express Relay on-chain stack allows searchers to perform liquidations across different protocols without needing to deploy their own contracts or perform bespoke integration work. The monitor service is important in enabling this, as it publishes the all the necessary information that searchers need for signing their intent on executing the liquidations.

Each protocol that integrates with Express Relay and the LiquidationAdapter workflow must provide code that publishes liquidation opportunities; the example file for the TokenVault dummy contract is found in `/protocols`. Some common types are defined in `utils/types_liquidation_adapter.py`, and standard functions for accessing Pyth prices can be found in `utils/pyth_prices.py`. The exact interface of the methods in the protocol's file is not important, but it should have a similar entrypoint with the same command line arguments and general behavior of sending liquidation opportunities to the liquidation server when specified.

The party that runs the monitor can run the protocol-provided file to surface liquidation opportunities to the liquidation server.
