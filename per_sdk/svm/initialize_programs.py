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

from per_sdk.svm.helpers import configure_logger, read_kp_from_json

logger = logging.getLogger(__name__)


def parse_args() -> argparse.Namespace:
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
    return parser.parse_args()


def create_ix_init_express_relay(
    pk_payer: Pubkey,
    pk_admin: Pubkey,
    pk_relayer_signer: Pubkey,
    pk_fee_receiver_relayer: Pubkey,
    express_relay_pid: Pubkey,
    split_router_default: int,
    split_relayer: int,
) -> Instruction:
    """
    Creates an instruction to initialize the express relay program.
    Args:
        pk_payer: Pubkey of the payer for the transaction
        pk_admin: Pubkey of the admin for the express relay program
        pk_relayer_signer: Pubkey of the relayer signer for the express relay program
        pk_fee_receiver_relayer: Pubkey of the fee receiver address for the relayer
        express_relay_pid: Pubkey of the express relay program
        split_router_default: Portion of bid that should go to router by default, in bps
        split_relayer: Portion of remaining bid (post protocol fee) that should go to relayer, in bps
    Returns:
        Instruction to initialize the express relay program
    """
    pk_express_relay_metadata = Pubkey.find_program_address(
        [b"metadata"], express_relay_pid
    )[0]
    discriminator_init_express_relay = hashlib.sha256(b"global:initialize").digest()[:8]
    data_init_express_relay = struct.pack(
        "<8sQQ",
        discriminator_init_express_relay,
        split_router_default,
        split_relayer,
    )
    return Instruction(
        express_relay_pid,
        data_init_express_relay,
        [
            AccountMeta(pk_payer, True, True),
            AccountMeta(pk_express_relay_metadata, False, True),
            AccountMeta(pk_admin, False, False),
            AccountMeta(pk_relayer_signer, False, False),
            AccountMeta(pk_fee_receiver_relayer, False, False),
            AccountMeta(system_pid, False, False),
        ],
    )


async def main():
    args = parse_args()

    configure_logger(logger, args.verbose)

    express_relay_pid = Pubkey.from_string(args.express_relay_program)

    kp_payer = read_kp_from_json(args.file_private_key_payer)
    pk_payer = kp_payer.pubkey()
    logger.info("Payer pubkey: %s", pk_payer)

    kp_admin = read_kp_from_json(args.file_private_key_admin)
    pk_admin = kp_admin.pubkey()
    logger.info("Admin pubkey: %s", pk_admin)

    kp_relayer_signer = read_kp_from_json(args.file_private_key_relayer_signer)
    pk_relayer_signer = kp_relayer_signer.pubkey()
    logger.info("Relayer signer pubkey: %s", pk_relayer_signer)

    client = AsyncClient(args.rpc_url, "confirmed")

    pk_express_relay_metadata = Pubkey.find_program_address(
        [b"metadata"], express_relay_pid
    )[0]
    balance_express_relay_metadata = await client.get_balance(pk_express_relay_metadata)

    tx = Transaction()
    if balance_express_relay_metadata.value == 0:
        ix_init_express_relay = create_ix_init_express_relay(
            pk_payer,
            pk_admin,
            pk_relayer_signer,
            pk_relayer_signer,
            express_relay_pid,
            args.split_protocol_default,
            args.split_relayer,
        )
        tx.add(ix_init_express_relay)

    if len(tx.instructions) > 0:
        tx_sig = (await client.send_transaction(tx, kp_payer)).value
        conf = await client.confirm_transaction(tx_sig)
        assert conf.value[0].status is None, "Initialization of programs failed"
        logger.info(f"Initialization of programs successful: {tx_sig}")
    else:
        logger.info("All programs already initialized")


if __name__ == "__main__":
    asyncio.run(main())
