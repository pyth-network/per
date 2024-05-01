import asyncio
from datetime import datetime
from web3 import AsyncWeb3
from web3.providers import WebsocketProviderV2


LOG = False  # toggle debug logging
if LOG:
    import logging
    logger = logging.getLogger("web3.providers.WebsocketProviderV2")
    logger.setLevel(logging.DEBUG)
    logger.addHandler(logging.StreamHandler())


urls = [
    "wss://sepolia.drpc.org",
    "wss://fantom.drpc.org",
    "wss://ethereum-rpc.publicnode.com",
    "wss://arbitrum-one-rpc.publicnode.com",
    "wss://fraa-dancebox-3043-rpc.a.dancebox.tanssi.network",
    "wss://optimism-rpc.publicnode.com",
    "wss://avalanche-c-chain-rpc.publicnode.com",
    "wss://mantle-rpc.publicnode.com",
    # - "wss://opbnb-mainnet.nodereal.io/ws/v1/e9a36765eb8a40b9bd12e680a1fd2bc5",
    # - "wss://aurora.drpc.org",
    # - "wss://mainnet.fusionnetwork.io",
    # - "wss://cronos.drpc.org",
    # - "wss://bsc-rpc.publicnode.com",
    # - "wss://polygon.drpc.org",
]


async def ws_v2_subscription_context_manager_example(socket_url, total=30):
    async with AsyncWeb3.persistent_websocket(
        WebsocketProviderV2(socket_url)
    ) as w3:
        count = 0
        # subscribe to new block headers
        await w3.eth.subscribe("newHeads")

        async for response in w3.ws.process_subscriptions():
            if count == 0:
                # Ignore the first block
                count += 1
                continue
            print(",".join([str(count), socket_url, str(datetime.now()), str(response["result"]["number"])]))
            count += 1
            if count >= total:
                break


async def run_all():
    await asyncio.gather(*[ws_v2_subscription_context_manager_example(socket_url) for socket_url in urls])


asyncio.run(run_all())
