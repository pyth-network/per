import web3
from eth_abi import encode
import json
from typing import TypedDict

from beacon.utils.pyth_prices import *
from beacon.utils.types_liquidation_adapter import *


"""
The protocol should implement a class called LiquidationAccount. This will be the type of the objects in the list returned by get_accounts() and fed into get_liquidatable.
This class should contain all the relevant information about a vault/account on this protocol that is necessary for identifying whether it is eligible for liquidation and constructing a LiquidationOpportunity object.
"""


class LiquidationAccount(TypedDict):
    # Keys of the TypedDict and their types
    pass


"""
get_accounts(rpc_url) is the first method that the protocol should implement. It should take the RPC URL of the chain as an argument and return all the open accounts in the protocol in the form of a list of objects of type LiquidationAccount (defined above). Each LiquidationAccount object represents an account/vault in the protocol.
This function can be implemented in any way, but it should be able to return all the open accounts in the protocol. For some protocols, this may be easily doable by just querying on-chain state; however, most protocols will likely need to maintain or access an off-chain indexer to get the list of all open accounts.
"""


async def get_accounts(rpc_url: str) -> list[LiquidationAccount]:
    # Fetch all vaults from on-chain state/indexer
    # Filter to just active vaults
    # Return list of LiquidationAccount
    # TODO: complete
    pass


"""
create_liquidation_opp is an optional helper function to construct a LiquidationOpportunity from a LiquidationAccount and a set of relevant Pyth PriceFeeds.
If you choose to implement this function, you can call it within get_liquidatable whenever you find a LiquidationAccount eligible for liquidation.
"""


def create_liquidation_opp(
        account: LiquidationAccount,
        prices: list[PriceFeed]) -> LiquidationOpportunity:
    pass


"""
get_liquidatable(accounts, prices) is the second method that the protocol should implement. It should take two arguments: account--a list of LiquidationAccount (defined above) objects--and prices--a dictionary of Pyth prices.
accounts should be the list of all open accounts in the protocol (i.e. the output of get_accounts()).
prices should be a dictionary of Pyth prices, where the keys are Pyth feed IDs and the values are PriceFeed objects. prices can be retrieved from the provided price retrieval functions.
This function should return a list of type LiquidationOpportunity.
"""


def get_liquidatable(accounts: list[LiquidationAccount],
                     prices: dict[str,
                                  PriceFeed]) -> (list[LiquidationOpportunity]):
    # Iterate through accounts
    # Determine if account is eligible for liquidation; if so, construct an object of type LiquidationOpportunity and add it to the list
    # Return the list of type LiquidationOpportunity containing all the valid liquidation opportunities
    pass


"""
The main loop below is a good mechanism to check if your implementations of the functions above are working properly.
"""


async def main():
    # get all accounts
    accounts = await get_accounts()

    # get prices
    pyth_price_feed_ids = await get_price_feed_ids()
    pyth_prices_latest = []
    i = 0
    cntr = 100
    while len(pyth_price_feed_ids[i:i + cntr]) > 0:
        pyth_prices_latest += await get_pyth_prices_latest(pyth_price_feed_ids[i:i + cntr])
        i += cntr
    pyth_prices_latest = dict(pyth_prices_latest)

    # get liquidatable accounts
    liquidatable = get_liquidatable(
        accounts, pyth_prices_latest)

    print(liquidatable)

if __name__ == "__main__":
    asyncio.run(main())
