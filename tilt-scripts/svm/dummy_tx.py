import argparse
import asyncio
import base64
import hashlib
import logging
import struct
import urllib
from pathlib import Path

import httpx
from solana.rpc.async_api import AsyncClient
from solana.rpc.commitment import Confirmed
from solana.transaction import Transaction
from solders.address_lookup_table_account import AddressLookupTableAccount
from solders.instruction import AccountMeta, Instruction
from solders.message import MessageV0
from solders.null_signer import NullSigner
from solders.pubkey import Pubkey
from solders.system_program import ID as system_pid
from solders.sysvar import INSTRUCTIONS as sysvar_ixs_pid
from solders.transaction import VersionedTransaction

from ..svm.helpers import configure_logger, read_kp_from_json

logger = logging.getLogger(__name__)

DEADLINE_MAX = 253402300799  # 9999-12-31T23:59:59Z


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("-v", "--verbose", action="count", default=0)
    parser.add_argument(
        "--file-private-key-searcher",
        type=Path,
        required=True,
        help="JSON file containing the private key (as a byte array) of the searcher for signing transaction",
    )
    parser.add_argument(
        "--file-private-key-relayer-signer",
        type=Path,
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
        "--use-lookup-table",
        action="store_true",
        default=False,
        help="Create and use a lookup table to store some commonly used accounts for the express relay program instruction when submitting to server. Used to test that server can handle querying accounts in lookup tables.",
    )
    parser.add_argument(
        "--submit-on-chain",
        action="store_true",
        default=False,
        help="Submit the transaction directly on-chain instead of submitting to the server",
    )
    parser.add_argument(
        "--use-legacy-transaction-bid",
        action="store_true",
        default=False,
        help="Use the legacy transaction message format instead of the versioned message format for the bid. Only applies if transaction is submitted as a bid (i.e. --submit-on-chain is not set)",
    )
    return parser.parse_args()


async def main():
    args = parse_args()

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
    router = Pubkey.find_program_address([b"fees_express_relay"], dummy_pid)[0]
    config_router = Pubkey.find_program_address(
        [b"config_router", bytes(router)], express_relay_pid
    )[0]
    pk_express_relay_metadata = Pubkey.find_program_address(
        [b"metadata"], express_relay_pid
    )[0]

    accounting = Pubkey.find_program_address([b"accounting"], dummy_pid)[0]

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
            AccountMeta(router, False, True),
            AccountMeta(config_router, False, False),
            AccountMeta(pk_express_relay_metadata, False, True),
            AccountMeta(pk_relayer_signer, False, True),
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
            AccountMeta(pk_express_relay_metadata, False, False),
            AccountMeta(sysvar_ixs_pid, False, False),
            AccountMeta(permission, False, False),
            AccountMeta(router, False, False),
            AccountMeta(config_router, False, False),
            AccountMeta(accounting, False, True),
            AccountMeta(system_pid, False, False),
        ],
    )

    client = AsyncClient(args.rpc_url, Confirmed)
    lookup_tables = []
    if args.use_lookup_table:
        address_lookup_pid = Pubkey.from_string(
            "AddressLookupTab1e1111111111111111111111111"
        )

        recent_slot = (await client.get_slot("finalized")).value

        (lookup_table_address, bump_lookup_table_address) = Pubkey.find_program_address(
            [bytes(pk_searcher), struct.pack("<Q", recent_slot)], address_lookup_pid
        )

        ix_create_lut = Instruction(
            address_lookup_pid,
            struct.pack("<LQB", 0, recent_slot, bump_lookup_table_address),
            [
                AccountMeta(lookup_table_address, False, True),
                AccountMeta(pk_searcher, True, False),
                AccountMeta(pk_searcher, True, True),
                AccountMeta(system_pid, False, False),
            ],
        )

        tx_create_lut = Transaction(fee_payer=pk_searcher)
        tx_create_lut.add(ix_create_lut)
        tx_create_lut_sig = (
            await client.send_transaction(tx_create_lut, kp_searcher)
        ).value
        conf_create_lut = await client.confirm_transaction(tx_create_lut_sig)

        assert (
            conf_create_lut.value[0].status is None
        ), "Create lookup table transaction failed"

        addresses_to_add = [router, config_router, pk_express_relay_metadata]
        addresses_to_add_bytes = [bytes(pubkey) for pubkey in addresses_to_add]
        data_extend = (
            struct.pack("<L", 2)
            + struct.pack("<Q", len(addresses_to_add_bytes))
            + b"".join(addresses_to_add_bytes)
        )
        ix_extend_lut = Instruction(
            address_lookup_pid,
            data_extend,
            [
                AccountMeta(lookup_table_address, False, True),
                AccountMeta(pk_searcher, True, False),
                AccountMeta(pk_searcher, True, True),
                AccountMeta(system_pid, False, False),
            ],
        )

        tx_extend_lut = Transaction(fee_payer=pk_searcher)
        tx_extend_lut.add(ix_extend_lut)
        tx_extend_lut_sig = (
            await client.send_transaction(tx_extend_lut, kp_searcher)
        ).value
        conf_extend_lut = await client.confirm_transaction(
            tx_extend_lut_sig, "finalized"
        )

        assert (
            conf_extend_lut.value[0].status is None
        ), "Extend lookup table transaction failed"

        logger.info(
            f"Extended lookup table at {lookup_table_address} with {addresses_to_add}"
        )

        lookup_tables.append(
            AddressLookupTableAccount(lookup_table_address, addresses_to_add)
        )

    if args.submit_on_chain:
        tx = Transaction(fee_payer=pk_searcher)
        tx.add(ix_submit_bid)
        tx.add(ix_dummy)
        tx_sig = (
            await client.send_transaction(tx, kp_searcher, kp_relayer_signer)
        ).value
        conf = await client.confirm_transaction(tx_sig)
        assert conf.value[0].status is None, "Transaction failed"
        logger.info(f"Submitted transaction with signature {tx_sig}")
    else:
        recent_blockhash = (await client.get_latest_blockhash()).value.blockhash
        if args.use_legacy_transaction_bid:
            tx = Transaction(fee_payer=pk_searcher, recent_blockhash=recent_blockhash)
            tx.add(ix_submit_bid)
            tx.add(ix_dummy)
            tx.sign_partial(kp_searcher)
            serialized = base64.b64encode(
                tx.serialize(verify_signatures=False)
            ).decode()
        else:
            messagev0 = MessageV0.try_compile(
                pk_searcher, [ix_submit_bid, ix_dummy], lookup_tables, recent_blockhash
            )
            signers = [kp_searcher, NullSigner(pk_relayer_signer)]
            partially_signed = VersionedTransaction(messagev0, signers)
            serialized = base64.b64encode(bytes(partially_signed)).decode()

        bid_body = {
            "chain_id": "development-solana-local",
            "transaction": serialized,
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
    asyncio.run(main())
