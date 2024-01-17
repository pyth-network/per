import httpx
import asyncio
from typing import TypedDict
import websockets
import json

HERMES_ENDPOINT = "https://hermes.pyth.network/api/"
HERMES_WS = "wss://hermes.pyth.network/"


class Price(TypedDict):
    price: str
    conf: str
    expo: int
    publish_time: int


class PriceFeed(TypedDict):
    id: str
    price: Price
    ema_price: Price
    vaa: str



async def get_price_feed_ids() -> list[str]:
    url = HERMES_ENDPOINT + "price_feed_ids"
    client = httpx.AsyncClient()
    
    data = (await client.get(url)).json()

    return data


class PriceFeedClient:
    def __init__(self, feed_ids: list[str]):
        self.feed_ids = feed_ids
        self.pending_feed_ids = feed_ids
        self.prices_dict: dict[str, PriceFeed] = {}
        self.client = httpx.AsyncClient()

    def add_feed_ids(self, feed_ids: list[str]):
        self.feed_ids += feed_ids
        self.feed_ids = list(set(self.feed_ids))
        self.pending_feed_ids += feed_ids

    def extract_price_feed(self, data: dict) -> PriceFeed:
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
    
    async def get_pyth_prices_latest(
        self,
        feedIds: list[str]
    ) -> list[tuple[str, PriceFeed]]:
        url = HERMES_ENDPOINT + "latest_price_feeds?"
        params = {"ids[]": feedIds, "binary": "true"}

        data = (await self.client.get(url, params=params)).json()

        results = []
        for res in data:
            price_feed = self.extract_price_feed(res)
            results.append((res['id'], price_feed))

        return results


    async def get_pyth_price_at_time(
        self,
        feed_id: str,
        timestamp: int
    ) -> tuple[str, PriceFeed]:
        url = HERMES_ENDPOINT + f"get_price_feed"
        params = {"id": feed_id, "publish_time": timestamp, "binary": "true"}

        data = (await self.client.get(url, params=params)).json()

        price_feed = self.extract_price_feed(data)

        return (feed_id, price_feed)


    async def get_all_prices(self) -> dict[str, PriceFeed]:
        pyth_prices_latest = []
        i = 0
        cntr = 100
        while len(self.feed_ids[i:i + cntr]) > 0:
            pyth_prices_latest += await self.get_pyth_prices_latest(self.feed_ids[i:i + cntr])
            i += cntr

        return dict(pyth_prices_latest)

    async def ws_pyth_prices(self):
        url_ws = "wss://hermes.pyth.network/ws"

        async with websockets.connect(url_ws) as ws:
            while True:
                if len(self.pending_feed_ids) > 0:
                    json_subscribe = {
                        "ids": self.pending_feed_ids,
                        "type": "subscribe",
                        "verbose": True,
                        "binary": True
                    }
                    await ws.send(json.dumps(json_subscribe))
                    self.pending_feed_ids = []

                msg = json.loads(await ws.recv())
                try:
                    if msg["type"] != "price_update":
                        continue

                    feed_id = msg["price_feed"]["id"]
                    new_feed = msg["price_feed"]

                    self.prices_dict[feed_id] = new_feed
                
                except:
                    raise Exception("Error in price_update message", msg)



    

async def main():
    feed_ids = await get_price_feed_ids()
    feed_ids = feed_ids[:1]
    price_feed_client = PriceFeedClient(feed_ids)

    print("Starting...")
    ws_call = price_feed_client.ws_pyth_prices()
    task = asyncio.create_task(ws_call)

    # Can insert continuous loop to check vaults
    while True:
        await asyncio.sleep(1)

if __name__ == "__main__":
    asyncio.run(main())