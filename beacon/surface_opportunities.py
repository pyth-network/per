import httpx
import asyncio
import argparse
import os

from beacon.protocols import beacon_TokenVault
from beacon.utils.pyth_prices import *
from beacon.utils.endpoints import *

PROTOCOLS = [beacon_TokenVault]


# TODO: turn on authorization in the surface post requests
async def main(operator_api_key: str, rpc_url: str):
    # get prices
    pyth_price_feed_ids = await get_price_feed_ids()
    pyth_prices_latest = []
    i = 0
    cntr = 100
    while len(pyth_price_feed_ids[i:i + cntr]) > 0:
        pyth_prices_latest += await get_pyth_prices_latest(pyth_price_feed_ids[i:i + cntr])
        i += cntr
    pyth_prices_latest = dict(pyth_prices_latest)

    liquidatable = []

    for protocol in PROTOCOLS:
        accounts = await protocol.get_accounts(rpc_url)

        liquidatable_protocol = protocol.get_liquidatable(
            accounts, pyth_prices_latest)

        liquidatable += liquidatable_protocol

    CLIENT = httpx.AsyncClient()

    resp = await CLIENT.post(
        f"{BEACON_SERVER_ENDPOINT_SURFACE}",
        json=liquidatable
    )

    print(f"Response PER post: {resp.text}")

if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--operator_api_key", type=str, required=True, help="Operator API key, used to authenticate the surface post request")
    parser.add_argument("--rpc_url", type=str, required=True, help="Chain RPC endpoint, used to fetch on-chain data via get_accounts")
    args = parser.parse_args()

    asyncio.run(main(args.operator_api_key, args.rpc_url))
