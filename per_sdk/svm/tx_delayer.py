import asyncio
import base64
import json
import random

from aiohttp import ClientSession, web
from solders.transaction import VersionedTransaction

PORT = 8080


async def proxy_to_rpc(data):
    rpc_url = "http://localhost:8899"

    # random latency between 0 and 5 seconds normal distribution
    random_seconds = random.gauss(5, 1.5)
    print(f"Sleeping for {random_seconds} seconds")
    await asyncio.sleep(max(random_seconds, 0))

    async with ClientSession() as session:
        resp = await session.post(rpc_url, json=data)
        print(await resp.text())


async def hello(request):
    data = await request.json()
    if data["method"] != "sendTransaction":
        return web.Response(text="Method not supported")
    # proxy to real rpc server in another thread
    asyncio.create_task(proxy_to_rpc(data))

    tx = VersionedTransaction.from_bytes(base64.b64decode(data["params"][0]))
    sig = tx.signatures[0]
    resposne = {"jsonrpc": "2.0", "result": str(sig), "id": data["id"]}

    return web.Response(text=json.dumps(resposne), content_type="application/json")


app = web.Application()
app.router.add_post("/", hello)

web.run_app(app, port=PORT)
