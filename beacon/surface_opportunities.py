import httpx
import asyncio

from beacon.protocols import beacon_TokenVault
from beacon.utils.pyth_prices import *
from beacon.utils.endpoints import *

# TODO: turn on authorization in the surface post requests
OPERATOR_API_KEY = "password"
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

        liquidatable_protocol = protocol.get_liquidatable(
            accounts, pyth_prices_latest)

        liquidatable += liquidatable_protocol

    CLIENT = httpx.AsyncClient()

    for item in liquidatable:
        resp = await CLIENT.post(
            f"{BEACON_SERVER_ENDPOINT_SURFACE}",
            json=item
        )
        print(f"Response PER post: {resp.text}")


if __name__ == "__main__":
    asyncio.run(main())
