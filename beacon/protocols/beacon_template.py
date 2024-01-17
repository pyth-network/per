import web3
from eth_abi import encode
import json
from typing import TypedDict
import argparse

from beacon.utils.pyth_prices import *
from beacon.utils.types_liquidation_adapter import *





class LiquidationAccount(TypedDict):
    """
    LiquidationAccount is a TypedDict that represents an account/vault in the protocol.

    The protocol should implement a class called LiquidationAccount. This will be the type of the objects in the list returned by get_accounts() and fed into get_liquidatable.
    This class should contain all the relevant information about a vault/account on this protocol that is necessary for identifying whether it is eligible for liquidation and constructing a LiquidationOpportunity object.
    """

    # Keys of the TypedDict and their types
    pass





async def get_accounts(rpc_url: str) -> list[LiquidationAccount]:
    """
    Returns all the open accounts in the protocol in the form of a list of type LiquidationAccount.

    get_accounts(rpc_url) is the first method that the protocol should implement. It should take the RPC URL of the chain as an argument and return all the open accounts in the protocol in the form of a list of objects of type LiquidationAccount (defined above). Each LiquidationAccount object represents an account/vault in the protocol.
    This function can be implemented in any way, but it should be able to return all the open accounts in the protocol. For some protocols, this may be easily doable by just querying on-chain state; however, most protocols will likely need to maintain or access an off-chain indexer to get the list of all open accounts.
    """    
    
    # Fetch all vaults from on-chain state/indexer
    # Filter to just active vaults
    # Return list of LiquidationAccount
    # TODO: complete
    pass



def create_liquidation_opp(
        account: LiquidationAccount,
        prices: list[PriceFeed]) -> LiquidationOpportunity:
    """
    Constructs a LiquidationOpportunity object from a LiquidationAccount object and a set of relevant Pyth PriceFeeds.

    This is an optional helper function you can implement. If you choose to do so, you can call it within get_liquidatable whenever you find a LiquidationAccount eligible for liquidation.
    """
    
    pass



def get_liquidatable(accounts: list[LiquidationAccount],
                     prices: dict[str,
                                  PriceFeed]) -> (list[LiquidationOpportunity]):
    """
    Filters list of LiquidationAccount types to return a list of LiquidationOpportunity types.

    get_liquidatable(accounts, prices) is the second method that the protocol should implement. It should take two arguments: account--a list of LiquidationAccount (defined above) objects--and prices--a dictionary of Pyth prices.
    accounts should be the list of all open accounts in the protocol (i.e. the output of get_accounts()).
    prices should be a dictionary of Pyth prices, where the keys are Pyth feed IDs and the values are PriceFeed objects. prices can be retrieved from the provided price retrieval functions.
    This function should return a list of type LiquidationOpportunity.
    """
   
    # Iterate through accounts
    # Determine if account is eligible for liquidation; if so, construct an object of type LiquidationOpportunity and add it to the list
    # Return the list of type LiquidationOpportunity containing all the valid liquidation opportunities
    pass




async def main():
    """
    main is a good mechanism to check if your implementations of the functions above are working properly.
    """
    parser = argparse.ArgumentParser()
    parser.add_argument("--operator_api_key", type=str, required=True, help="Operator API key, used to authenticate the surface post request")
    parser.add_argument("--rpc_url", type=str, required=True, help="Chain RPC endpoint, used to fetch on-chain data via get_accounts")
    parser.add_argument("--beacon_server_url", type=str, help="Beacon server endpoint; if provided, will send liquidation opportunities to the beacon server; otherwise, will just print them out")
    args = parser.parse_args()

    # get prices
    feed_ids = [] # TODO: specify initial price feeds to subscribe to
    price_feed_client = PriceFeedClient(feed_ids)

    ws_call = price_feed_client.ws_pyth_prices()
    task = asyncio.create_task(ws_call)

    client = httpx.AsyncClient()

    await asyncio.sleep(2)

    while True:
        # get all accounts
        accounts = await get_accounts(args.rpc_url)

        liquidatable = get_liquidatable(accounts, price_feed_client.prices_dict)
        
        if args.beacon_server_url:
            # this post request will not work without an operator API key; however, this should work fine if get_liquidatable returns the correct type
            resp = await client.post(
                args.beacon_server_url,
                json=liquidatable
            )
            print(f"Response, post to beacon: {resp.text}")
        else:
            print(liquidatable)
        await asyncio.sleep(2)

if __name__ == "__main__":
    asyncio.run(main())
