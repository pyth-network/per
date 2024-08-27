import argparse
import hashlib
import logging
import struct
import urllib

import httpx
from solana.rpc.async_api import AsyncClient
from solana.transaction import Transaction
from solders.instruction import AccountMeta, Instruction
from solders.message import Message
from solders.pubkey import Pubkey
from solders.system_program import ID as system_pid
from solders.sysvar import INSTRUCTIONS as sysvar_ixs_pid

from per_sdk.svm.helpers import configure_logger, read_kp_from_json

logger = logging.getLogger(__name__)

DEADLINE_MAX = 2**63 - 1


async def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("-v", "--verbose", action="count", default=0)
    parser.add_argument(
        "--file-private-key-searcher",
        type=str,
        required=True,
        help="JSON file containing the private key (as a byte array) of the searcher for signing transaction",
    )
    parser.add_argument(
        "--file-private-key-relayer-signer",
        type=str,
        required=True,
        help="JSON file containing the private key (as a byte array) of the relayer signer",
    )
    parser.add_argument(
        "--bid",
        type=int,
        default=int(1),
        help="Default amount of bid",
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
        "--express-relay-program",
        type=str,
        required=True,
        help="Pubkey of the express relay program, as a base-58-encoded string",
    )
    parser.add_argument(
        "--dummy-program",
        type=str,
        required=True,
        help="Pubkey of the dummy program, as a base-58-encoded string",
    )
    parser.add_argument(
        "--submit-on-chain",
        action="store_true",
        default=False,
        help="Submit the transaction directly on-chain instead of submitting to the server",
    )
    args = parser.parse_args()

    configure_logger(logger, args.verbose)

    express_relay_pid = Pubkey.from_string(args.express_relay_program)
    dummy_pid = Pubkey.from_string(args.dummy_program)

    kp_searcher = read_kp_from_json(args.file_private_key_searcher)
    pk_searcher = kp_searcher.pubkey()
    logger.info("Searcher pubkey: %s", pk_searcher)

    kp_relayer_signer = read_kp_from_json(args.file_private_key_relayer_signer)
    pk_relayer_signer = kp_relayer_signer.pubkey()
    logger.info("Relayer signer pubkey: %s", pk_relayer_signer)

    permission = Pubkey.find_program_address([b"vault"], dummy_pid)[0]
    protocol_config = Pubkey.find_program_address(
        [b"config_protocol", bytes(dummy_pid)], express_relay_pid
    )[0]
    pk_fee_receiver_dummy = Pubkey.find_program_address(
        [b"express_relay_fees"], dummy_pid
    )[0]
    pk_express_relay_metadata = Pubkey.find_program_address(
        [b"metadata"], express_relay_pid
    )[0]

    discriminator_submit_bid = hashlib.sha256(b"global:submit_bid").digest()[:8]
    data_submit_bid = struct.pack(
        "<8sqQ", discriminator_submit_bid, DEADLINE_MAX, args.bid
    )
    ix_submit_bid = Instruction(
        express_relay_pid,
        data_submit_bid,
        [
            AccountMeta(pk_searcher, True, True),
            AccountMeta(pk_relayer_signer, True, False),
            AccountMeta(permission, False, False),
            AccountMeta(dummy_pid, False, False),
            AccountMeta(protocol_config, False, False),
            AccountMeta(pk_relayer_signer, False, True),
            AccountMeta(pk_fee_receiver_dummy, False, True),
            AccountMeta(pk_express_relay_metadata, False, True),
            AccountMeta(system_pid, False, False),
            AccountMeta(sysvar_ixs_pid, False, False),
        ],
    )

    discriminator_do_nothing = hashlib.sha256(b"global:do_nothing").digest()[:8]
    data_do_nothing = struct.pack("<8s", discriminator_do_nothing)
    ix_dummy = Instruction(
        dummy_pid,
        data_do_nothing,
        [
            AccountMeta(pk_searcher, True, True),
            AccountMeta(express_relay_pid, False, False),
            AccountMeta(sysvar_ixs_pid, False, False),
            AccountMeta(permission, False, False),
            AccountMeta(dummy_pid, False, False),
        ],
    )

    tx = Transaction()
    tx.add(ix_submit_bid)
    tx.add(ix_dummy)

    if args.submit_on_chain:
        client = AsyncClient(args.rpc_url, "confirmed")
        tx_sig = (
            await client.send_transaction(tx, kp_searcher, kp_relayer_signer)
        ).value
        conf = await client.confirm_transaction(tx_sig)
        assert conf.value[0].status is None, "Transaction failed"
        logger.info(f"Submitted transaction with signature {tx_sig}")
    else:
        tx.sign_partial(kp_searcher)
        message = bytes(Message([ix_submit_bid, ix_dummy], pk_searcher))
        # TODO: impute one signature into the message
        bid_body = {
            "permission_key": str(permission),
            "chain_id": "solana",
            "amount": args.bid,
            "transaction": message.hex(),
        }
        client = httpx.AsyncClient()
        resp = await client.post(
            urllib.parse.urljoin(
                args.auction_server_url,
                "v1/bids",
            ),
            json=bid_body,
            timeout=20,
        )
        logger.info(
            f"Submitted bid amount {args.bid} on permission key {str(permission)}, server response: {resp.text}"
        )


if __name__ == "__main__":
    import asyncio

    asyncio.run(main())
