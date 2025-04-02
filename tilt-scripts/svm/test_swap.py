import argparse
import asyncio
import base64
import logging
import random
from pathlib import Path

import httpx
from solana.rpc.api import Client as SolanaClient
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


async def send_and_submit_quote(
    server_url, rpc_url, kp_taker, input_token, output_token, side, cancellable
):
    chain_id = "local-solana"
    pk_taker = kp_taker.pubkey()
    logger.info("Taker pubkey: %s", pk_taker)
    payload = {
        "chain_id": chain_id,
        "input_token_mint": input_token,
        "output_token_mint": output_token,
        "router": "3hv8L8UeBbyM3M25dF3h2C5p8yA4FptD7FFZu4Z1jCMn",
        "referral_fee_bps": 10,
        "specified_token_amount": {"amount": random.randint(1, 1000), "side": side},
        "user_wallet_address": str(pk_taker),
        "memo": "memo",
        "version": "v1",
        "cancellable": cancellable,
    }

    async with httpx.AsyncClient() as http_client:
        result = await http_client.post(
            server_url + "/v1/opportunities/quote", json=payload
        )
        if result.status_code != 200:
            logger.error("Failed to get quote from auction server %s", result.text)
            return
        logger.info("Input token %s", result.json()["input_token"])
        logger.info("Output token %s", result.json()["output_token"])
        logger.info("Referrer fee %s", result.json()["referrer_fee"])
        logger.info("Platform fee %s", result.json()["platform_fee"])
        response = result.json()
        logger.info(response)
        tx = SoldersTransaction.from_bytes(base64.b64decode(response["transaction"]))
        accounts = tx.message.account_keys
        tx = Transaction.from_solders(tx)
        tx.sign_partial(kp_taker)

        if cancellable:
            position = accounts.index(pk_taker)
            reference_id = response["reference_id"]

            payload = {
                "reference_id": reference_id,
                "user_signature": str(tx.signatures[position]),
            }
            await asyncio.sleep(0.5)
            result = await http_client.post(
                server_url + "/v1/{}/quotes/submit".format(chain_id),
                json=payload,
            )
            if result.status_code != 200:
                logger.error("Failed to submit quote to auction server %s", result.text)
                return

            response = result.json()
            tx = tx = SoldersTransaction.from_bytes(
                base64.b64decode(response["transaction"])
            )
            logger.info("Quote submitted to server. Signature: %s", tx.signatures[0])
        else:
            client = SolanaClient(rpc_url)
            client.send_raw_transaction(tx.serialize())
            logger.info("Transaction broadcasted. Signature: %s", tx.signatures[0])


async def main():
    args = parse_args()

    configure_logger(logger, args.verbose)

    input_mint = args.input_mint
    output_mint = args.output_mint
    native_token_address = "So11111111111111111111111111111111111111112"
    server_url = args.auction_server_url
    rpc_url = args.rpc_url
    kp_taker = read_kp_from_json(args.file_private_key_taker)

    for input_token in [input_mint, native_token_address]:
        for output_token in [output_mint, native_token_address]:
            if input_token != output_token:
                for side in ["input", "output"]:
                    for cancellable in [True, False]:
                        await send_and_submit_quote(
                            server_url,
                            rpc_url,
                            kp_taker,
                            input_token,
                            output_token,
                            side,
                            cancellable,
                        )


if __name__ == "__main__":
    asyncio.run(main())
