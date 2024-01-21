import httpx
import asyncio
from typing import TypedDict

HERMES_ENDPOINT_HTTPS = "https://hermes.pyth.network/api/"
HERMES_ENDPOINT_WSS = "wss://hermes.pyth.network/ws"


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
    """
    Queries the Hermes https endpoint for a list of the IDs of all Pyth price feeds.
    """
    
    url = HERMES_ENDPOINT_HTTPS + "price_feed_ids"
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
        """
        Extracts a PriceFeed object from the JSON response from Hermes.
        """
        price = data['price']
        price_ema = data['ema_price']
        vaa = data['vaa']
        price_feed = {
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
        """
        Queries the Hermes https endpoint for the latest price feeds for a list of Pyth feed IDs.
        """
        url = HERMES_ENDPOINT_HTTPS + "latest_price_feeds?"
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
        """
        Queries the Hermes https endpoint for the price feed for a Pyth feed ID at a given timestamp.
        """
        url = HERMES_ENDPOINT_HTTPS + f"get_price_feed"
        params = {"id": feed_id, "publish_time": timestamp, "binary": "true"}

        data = (await self.client.get(url, params=params)).json()

        price_feed = self.extract_price_feed(data)

        return (feed_id, price_feed)

    async def get_all_prices(self) -> dict[str, PriceFeed]:
        """
        Queries the Hermes http endpoint for the latest price feeds for all feed IDs in the class object.
        
        There are limitations on the number of feed IDs that can be queried at once, so this function queries the feed IDs in batches.
        """
        pyth_prices_latest = []
        i = 0
        batch_size = 100
        while len(self.feed_ids[i:i + batch_size]) > 0:
            pyth_prices_latest += await self.get_pyth_prices_latest(self.feed_ids[i:i + batch_size])
            i += batch_size

        return dict(pyth_prices_latest)

    async def ws_pyth_prices(self):
        """
        Opens a websocket connection to Hermes for latest prices for all feed IDs in the class object.
        """
        import json
        import websockets

        async with websockets.connect(HERMES_ENDPOINT_WSS) as ws:
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
                    if msg["type"] == "response":
                        if msg["status"] != "success":
                            raise Exception("Error in subscribing to websocket")
                    if msg["type"] != "price_update":
                        continue

                    feed_id = msg["price_feed"]["id"]
                    new_feed = msg["price_feed"]

                    self.prices_dict[feed_id] = new_feed

                except:
                    raise Exception("Error in price_update message", msg)


async def main():
    feed_ids = await get_price_feed_ids()
    feed_ids = feed_ids[:1] # TODO: remove this line, once rate limits are figured out
    price_feed_client = PriceFeedClient(feed_ids)

    print("Starting web socket...")
    ws_call = price_feed_client.ws_pyth_prices()
    asyncio.create_task(ws_call)

    while True:
        await asyncio.sleep(1)

if __name__ == "__main__":
    asyncio.run(main())
