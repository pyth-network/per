import argparse
import asyncio
import logging

from express_relay.svm.generated.express_relay.instructions.initialize import (
    InitializeAccounts,
    initialize,
)
from express_relay.svm.generated.express_relay.instructions.set_swap_platform_fee import (
    SetSwapPlatformFeeAccounts,
    set_swap_platform_fee,
)
from express_relay.svm.generated.express_relay.types.initialize_args import (
    InitializeArgs,
)
from express_relay.svm.generated.express_relay.types.set_swap_platform_fee_args import (
    SetSwapPlatformFeeArgs,
)
from solana.rpc.async_api import AsyncClient
from solana.rpc.commitment import Confirmed
from solana.transaction import Transaction
from solders.pubkey import Pubkey

from ..svm.helpers import configure_logger, read_kp_from_json

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
        "--swap-platform-fee-bps",
        type=int,
        required=False,
        default=10,
        help="The portion of the swap amount that should go to the platform (relayer + express relay), in bps",
    )
    parser.add_argument(
        "--rpc-url",
        type=str,
        required=False,
        default="http://localhost:8899",
        help="URL of the Solana RPC endpoint to use for submitting transactions",
    )
    return parser.parse_args()


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

    client = AsyncClient(args.rpc_url, Confirmed)

    pk_express_relay_metadata = Pubkey.find_program_address(
        [b"metadata"], express_relay_pid
    )[0]
    balance_express_relay_metadata = await client.get_balance(pk_express_relay_metadata)

    tx = Transaction()
    signers = [kp_admin]
    if balance_express_relay_metadata.value == 0:
        ix_init_express_relay = initialize(
            {
                "data": InitializeArgs(
                    split_router_default=args.split_protocol_default,
                    split_relayer=args.split_relayer,
                ),
            },
            InitializeAccounts(
                payer=pk_payer,
                express_relay_metadata=pk_express_relay_metadata,
                admin=pk_admin,
                relayer_signer=pk_relayer_signer,
                fee_receiver_relayer=pk_relayer_signer,
            ),
            program_id=express_relay_pid,
        )
        tx.add(ix_init_express_relay)
        signers.append(kp_payer)

    ix_set_swap_platform_fee = set_swap_platform_fee(
        {
            "data": SetSwapPlatformFeeArgs(
                swap_platform_fee_bps=args.split_protocol_default
            ),
        },
        SetSwapPlatformFeeAccounts(
            admin=pk_admin,
            express_relay_metadata=pk_express_relay_metadata,
        ),
        program_id=express_relay_pid,
    )
    tx.add(ix_set_swap_platform_fee)

    if len(tx.instructions) > 0:
        tx_sig = (await client.send_transaction(tx, *signers)).value
        conf = await client.confirm_transaction(tx_sig)
        assert conf.value[0].status is None, "Initialization of programs failed"
        logger.info(f"Initialization of programs successful: {tx_sig}")
    else:
        logger.info("All programs already initialized")


if __name__ == "__main__":
    asyncio.run(main())
