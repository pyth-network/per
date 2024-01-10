import httpx
import asyncio

from pythresearch.per.beacon.protocols import beacon_TokenVault
from pythresearch.per.beacon.utils.pyth_prices import *
from pythresearch.per.beacon.utils.endpoints import *

OPERATOR_API_KEY = "password" ## TODO: turn on authorization in the surface post requests
PROTOCOLS = [beacon_TokenVault]

async def main():
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
        accounts = await protocol.get_accounts()

        liquidatable_protocol = protocol.get_liquidatable(accounts, pyth_prices_latest)
        
        liquidatable += liquidatable_protocol

    CLIENT = httpx.AsyncClient()

    resp = await CLIENT.post(
        f"{BEACON_SERVER_ENDPOINT_SURFACE}",
        json=liquidatable
    )

    print(f"Response PER post: {resp.text}")

if __name__ == "__main__":
    asyncio.run(main())