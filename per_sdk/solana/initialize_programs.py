import argparse
import asyncio
import hashlib
import logging
import struct

from solana.rpc.async_api import AsyncClient
from solana.transaction import Transaction
from solders.instruction import AccountMeta, Instruction
from solders.pubkey import Pubkey
from solders.system_program import ID as system_pid

from per_sdk.solana.helpers import read_kp_from_json

logger = logging.getLogger(__name__)


async def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("-v", "--verbose", action="count", default=0)
    parser.add_argument(
        "--file-private-key-payer",
        type=str,
        required=True,
        help="JSON file containing the private key (as a byte array) of the payer for signing transaction",
    )
    parser.add_argument(
        "--file-private-key-admin",
        type=str,
        required=True,
        help="JSON file containing the private key (as a byte array) of the admin for express relay",
    )
    parser.add_argument(
        "--file-private-key-relayer-signer",
        type=str,
        required=True,
        help="JSON file containing the private key (as a byte array) of the relayer signer for express relay",
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
        "--split-protocol-default",
        type=int,
        required=False,
        default=4000,
        help="Percentage of bid that should go to protocol by default, in bps",
    )
    parser.add_argument(
        "--split-relayer",
        type=int,
        required=False,
        default=2000,
        help="Percentage of remaining bid (post protocol fee) that should go to relayer, in bps",
    )
    parser.add_argument(
        "--rpc-url",
        type=str,
        required=False,
        default="http://localhost:8899",
        help="URL of the Solana RPC endpoint to use for submitting transactions",
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

    express_relay_pid = Pubkey.from_string(args.express_relay_program)
    dummy_pid = Pubkey.from_string(args.dummy_program)

    kp_payer = read_kp_from_json(args.file_private_key_payer)
    pk_payer = kp_payer.pubkey()
    logger.info("Payer pubkey: %s", pk_payer)

    kp_admin = read_kp_from_json(args.file_private_key_admin)
    pk_admin = kp_admin.pubkey()
    logger.info("Admin pubkey: %s", pk_admin)

    kp_relayer_signer = read_kp_from_json(args.file_private_key_relayer_signer)
    pk_relayer_signer = kp_relayer_signer.pubkey()
    logger.info("Relayer signer pubkey: %s", pk_relayer_signer)

    pk_express_relay_metadata = Pubkey.find_program_address(
        [b"metadata"], express_relay_pid
    )[0]
    discriminator_init_express_relay = hashlib.sha256(b"global:initialize").digest()[:8]
    data_init_express_relay = struct.pack(
        "<8sQQ",
        discriminator_init_express_relay,
        args.split_protocol_default,
        args.split_relayer,
    )
    ix_init_express_relay = Instruction(
        express_relay_pid,
        data_init_express_relay,
        [
            AccountMeta(pk_payer, True, True),
            AccountMeta(pk_express_relay_metadata, False, True),
            AccountMeta(pk_admin, False, False),
            AccountMeta(pk_relayer_signer, False, False),
            AccountMeta(pk_relayer_signer, False, False),
            AccountMeta(system_pid, False, False),
        ],
    )

    pk_fee_receiver_dummy = Pubkey.find_program_address(
        [b"express_relay_fees"], dummy_pid
    )[0]
    discriminator_init_dummy = hashlib.sha256(b"global:initialize").digest()[:8]
    data_init_dummy = struct.pack("<8s", discriminator_init_dummy)
    ix_init_dummy = Instruction(
        dummy_pid,
        data_init_dummy,
        [
            AccountMeta(pk_payer, True, True),
            AccountMeta(pk_fee_receiver_dummy, False, True),
            AccountMeta(system_pid, False, False),
        ],
    )

    client = AsyncClient(args.rpc_url)
    balance_express_relay_metadata = await client.get_balance(pk_express_relay_metadata)
    balance_fee_receiver_dummy = await client.get_balance(pk_fee_receiver_dummy)

    tx = Transaction()
    if balance_express_relay_metadata.value == 0:
        tx.add(ix_init_express_relay)
    if balance_fee_receiver_dummy.value == 0:
        tx.add(ix_init_dummy)
    if len(tx.instructions) > 0:
        tx_sig = (await client.send_transaction(tx, kp_payer)).value
        conf = await client.confirm_transaction(tx_sig)
        assert conf.value[0].status is None, "Initialization of programs failed"
        logger.info(f"Initialization of programs successful: {tx_sig}")
    else:
        logger.info("All programs already initialized")


if __name__ == "__main__":
    asyncio.run(main())
