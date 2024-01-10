import httpx
import asyncio
from typing import TypedDict

HERMES_ENDPOINT = "https://hermes.pyth.network/api/"


class Price(TypedDict):
    price: int
    conf: int
    expo: int
    publish_time: int


class PriceFeed(TypedDict):
    feed_id: str
    price: Price
    price_ema: Price
    vaa: str


CLIENT = httpx.AsyncClient()


def extract_price_feed(data: dict) -> PriceFeed:
    price: Price = data['price']
    price_ema: Price = data['ema_price']
    vaa = data['vaa']
    price_feed: PriceFeed = {
        "feed_id": data['id'],
        "price": price,
        "price_ema": price_ema,
        "vaa": vaa
    }
    return price_feed


async def get_price_feed_ids() -> list[str]:
    url = HERMES_ENDPOINT + "price_feed_ids"

    data = (await CLIENT.get(url)).json()

    return data


async def get_pyth_prices_latest(
    feedIds: list[str]
) -> list[tuple[str, PriceFeed]]:
    url = HERMES_ENDPOINT + "latest_price_feeds?"
    params = {"ids[]": feedIds, "binary": "true"}

    data = (await CLIENT.get(url, params=params)).json()

    results = []
    for res in data:
        price_feed = extract_price_feed(res)
        results.append((res['id'], price_feed))

    return results


async def get_pyth_price_at_time(
    feed_id: str,
    timestamp: int
) -> tuple[str, PriceFeed]:
    url = HERMES_ENDPOINT + f"get_price_feed"
    params = {"id": feed_id, "publish_time": timestamp, "binary": "true"}

    data = (await CLIENT.get(url, params=params)).json()

    price_feed = extract_price_feed(data)

    return (feed_id, price_feed)


async def get_all_prices() -> dict[str, PriceFeed]:
    pyth_price_feed_ids = await get_price_feed_ids()

    pyth_prices_latest = []
    i = 0
    cntr = 100
    while len(pyth_price_feed_ids[i:i + cntr]) > 0:
        pyth_prices_latest += await get_pyth_prices_latest(pyth_price_feed_ids[i:i + cntr])
        i += cntr

    return dict(pyth_prices_latest)


async def main():
    pyth_price = await get_pyth_price_at_time("0xff61491a931112ddf1bd8147cd1b641375f79f5825126d665480874634fd0ace", 1703016621)

    data = await get_all_prices()

    return pyth_price, data

if __name__ == "__main__":
    pyth_price, data = asyncio.run(main())

    import pdb
    pdb.set_trace()
