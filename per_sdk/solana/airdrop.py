import argparse
import asyncio
import json
import logging
import os

from solana.rpc.async_api import AsyncClient
from solders.keypair import Keypair

from per_sdk.solana.helpers import read_kp_from_json

logger = logging.getLogger(__name__)


async def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("-v", "--verbose", action="count", default=0)
    parser.add_argument(
        "--rpc-url",
        type=str,
        required=False,
        default="http://localhost:8899",
        help="URL of the Solana RPC endpoint to use for submitting transactions",
    )
    parser.add_argument(
        "--airdrop-amount",
        type=int,
        required=False,
        default=10**9,
        help="Amount of lamports to airdrop to the keypairs",
    )
    args = parser.parse_args()

    logger.setLevel(logging.INFO if args.verbose == 0 else logging.DEBUG)
    log_handler = logging.StreamHandler()
    formatter = logging.Formatter(
        "%(asctime)s %(levelname)s:%(name)s:%(module)s %(message)s",
        datefmt="%Y-%m-%d %H:%M:%S",
    )
    log_handler.setFormatter(formatter)
    logger.addHandler(log_handler)

    client = AsyncClient(args.rpc_url)

    for account in ["searcher", "admin", "relayer_signer"]:
        file_path = f"keypairs/{account}.json"
        if not os.path.exists(file_path):
            kp = Keypair()
            with open(file_path, "w") as f:
                json.dump(kp.to_bytes_array(), f)
                logger.debug(f"Created and saved {account} keypair")
        else:
            logger.debug(f"Reusing existing {account} keypair")
            kp = read_kp_from_json(file_path)
        airdrop_sig = (
            await client.request_airdrop(kp.pubkey(), args.airdrop_amount)
        ).value
        conf = await client.confirm_transaction(airdrop_sig)
        assert conf.value[0].status is None, f"Airdrop to {account} failed"
        logger.info(f"Airdrop to {account} successful")


if __name__ == "__main__":
    asyncio.run(main())
