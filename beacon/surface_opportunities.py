import httpx
import asyncio

from pythresearch.per.beacon.protocols import beacon_TokenVault
from pythresearch.per.beacon.utils.pyth_prices import *
from pythresearch.per.beacon.utils.endpoints import *

OPERATOR_API_KEY = "password"
BEACONS = [beacon_TokenVault]

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

    liquidatable_permissionless = []
    liquidatable_per = []

    for beacon in BEACONS:
        accounts = await beacon.get_accounts()

        liquidatable_permissionless_protocol, liquidatable_per_protocol = beacon.get_liquidatable(
            accounts, pyth_prices_latest)

        liquidatable_permissionless += liquidatable_permissionless_protocol
        liquidatable_per += liquidatable_per_protocol

    CLIENT = httpx.AsyncClient()

    ## TODO: fill out these submissions to beacon server
    await CLIENT.post(f"{BEACON_SERVER_ENDPOINT}/PER", )
    await CLIENT.post(f"{BEACON_SERVER_ENDPOINT}/permissionless")

if __name__ == "__main__":
    asyncio.run(main())