import argparse
import asyncio
import base64
import logging
import random
from pathlib import Path

import httpx
from solana.rpc.async_api import AsyncClient
from solana.rpc.commitment import Confirmed
from solana.transaction import Transaction
from solders.transaction import Transaction as SoldersTransaction

from ..svm.helpers import configure_logger, read_kp_from_json

logger = logging.getLogger(__name__)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("-v", "--verbose", action="count", default=0)
    parser.add_argument(
        "--file-private-key-taker",
        type=Path,
        required=True,
        help="JSON file containing the private key (as a byte array) of the taker for swap transaction",
    )
    parser.add_argument(
        "--auction-server-url",
        type=str,
        required=True,
        help="Auction server endpoint to use for submitting bids",
    )
    parser.add_argument(
        "--rpc-url",
        type=str,
        required=False,
        default="http://localhost:8899",
        help="URL of the Solana RPC endpoint to use for submitting transactions",
    )
    parser.add_argument(
        "--input-mint",
        type=str,
        required=True,
        help="Input mint",
    )
    parser.add_argument(
        "--output-mint",
        type=str,
        required=True,
        help="Output mint",
    )
    return parser.parse_args()


async def main():
    args = parse_args()

    configure_logger(logger, args.verbose)

    kp_taker = read_kp_from_json(args.file_private_key_taker)
    pk_taker = kp_taker.pubkey()
    logger.info("Taker pubkey: %s", pk_taker)
    payload = {
        "chain_id": "local-solana",
        "input_token_mint": args.input_mint,
        "output_token_mint": args.output_mint,
        "router": "3hv8L8UeBbyM3M25dF3h2C5p8yA4FptD7FFZu4Z1jCMn",
        "referral_fee_bps": 10,
        "specified_token_amount": {"amount": random.randint(1, 1000), "side": "output"},
        "user_wallet_address": str(pk_taker),
        "version": "v1",
    }
    async with httpx.AsyncClient() as http_client:
        result = await http_client.post(
            args.auction_server_url + "/v1/opportunities/quote", json=payload
        )
        if result.status_code != 200:
            logger.error("Failed to get quote from auction server %s", result.text)
            return
        logger.info("Input token %s", result.json()["input_token"])
        logger.info("Output token %s", result.json()["output_token"])
        tx = SoldersTransaction.from_bytes(
            base64.b64decode(result.json()["transaction"])
        )
        tx = Transaction.from_solders(tx)
        tx.sign_partial(kp_taker)
        async with AsyncClient(args.rpc_url) as rpc_client:
            await rpc_client.send_raw_transaction(tx.serialize())
            logger.info("Swap transaction sent. Signature: %s", tx.signatures[0])
            await rpc_client.confirm_transaction(tx.signatures[0], commitment=Confirmed)
            logger.info("Swap transaction confirmed")


if __name__ == "__main__":
    asyncio.run(main())
