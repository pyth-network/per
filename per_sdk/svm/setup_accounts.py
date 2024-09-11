import argparse
import asyncio
import json
import logging
from pathlib import Path

from solana.rpc.async_api import AsyncClient
from solana.rpc.commitment import Confirmed
from solders.keypair import Keypair

from per_sdk.svm.helpers import configure_logger, read_kp_from_json

logger = logging.getLogger(__name__)


def parse_args() -> argparse.Namespace:
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
    return parser.parse_args()


async def main():
    args = parse_args()

    configure_logger(logger, args.verbose)

    client = AsyncClient(args.rpc_url, Confirmed)

    keypairs_dir = Path("keypairs/")
    if not keypairs_dir.exists():
        keypairs_dir.mkdir(exist_ok=True, parents=True)

    for account in ["searcher", "admin", "relayer_signer"]:
        file_path = keypairs_dir / f"{account}.json"
        if not file_path.exists():
            kp = Keypair()
            with file_path.open("w") as f:
                json.dump(kp.to_bytes_array(), f)
                logger.info(f"Created and saved {account} keypair")
        else:
            logger.info(f"Reusing existing {account} keypair")
            kp = read_kp_from_json(file_path)
        airdrop_sig = (
            await client.request_airdrop(kp.pubkey(), args.airdrop_amount)
        ).value
        conf = await client.confirm_transaction(airdrop_sig)
        assert conf.value[0].status is None, f"Airdrop to {account} failed"
        logger.info(f"Airdrop to {account} successful")


if __name__ == "__main__":
    asyncio.run(main())
