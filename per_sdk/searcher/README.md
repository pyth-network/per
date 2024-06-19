# Searcher Integration

Searchers can integrate with Express Relay by one of two means:

1. Simple integration with LiquidationAdapter via an Externally Owned Account (EOA)
2. Advanced integration with PER using a customized searcher contract

Option 2 requires searchers to handle individual protocol interfaces and smart contract risk, and it is similar in nature to how many searchers currently do liquidations via their own deployed contracts--searchers can now call into their smart contracts via the Express Relay workflow. This option allows for greater customization by the searcher, but requires additional work per each protocol that the searcher wants to integrate with.

Meanwhile, option 1 requires much less work and does not require contract deployment by the searcher. For option 1, the searcher submits their bid on the liquidation opportunity to the liquidation server, which handles transaction submission, routing the liquidation logic to the protocol and also performs safety checks to ensure that the searcher is paying and receiving the appropriate amounts, as specified in the liquidation opportunity structure. The searcher can submit transactions signed by their EOA that has custody of the tokens they wish to repay with. Searchers can listen to liquidation opportunities using the liquidation server, and if they wish to bid on a liquidation opportunity, they can submit it via the same server. Helper functions related to constructing the signature for the LiquidationAdapter contract are in `searcher_utils.py`. A sample workflow is in `simple_searcher.py` (note: this example lacks any serious evaluation of opportunities, and it simply carries out a liquidation if the opportunity is available).
