import argparse
import asyncio
import logging
import typing
from decimal import Decimal
from typing import List

from express_relay.client import ExpressRelayClient
from express_relay.constants import SVM_CONFIGS
from express_relay.models import BidStatusUpdate, Opportunity, OpportunityDelete
from express_relay.models.base import BidStatusVariantsSvm
from express_relay.models.svm import (
    BidSvm,
    LimoOpportunitySvm,
    OnChainBidSvm,
    OpportunitySvm,
    SvmChainUpdate,
    SwapBidSvm,
    SwapOpportunitySvm,
)
from express_relay.svm.generated.express_relay.accounts.express_relay_metadata import (
    ExpressRelayMetadata,
)
from express_relay.svm.generated.express_relay.program_id import (
    PROGRAM_ID as SVM_EXPRESS_RELAY_PROGRAM_ID,
)
from express_relay.svm.limo_client import LimoClient, OrderStateAndAddress
from solana.rpc.async_api import AsyncClient
from solana.rpc.commitment import Finalized
from solders.compute_budget import set_compute_unit_price
from solders.instruction import Instruction
from solders.keypair import Keypair
from solders.pubkey import Pubkey
from solders.transaction import Transaction

DEADLINE = 2 * 10**10


class SimpleSearcherSvm:
    express_relay_metadata: ExpressRelayMetadata | None
    mint_decimals_cache: typing.Dict[str, int]
    latest_chain_update: typing.Dict[str, SvmChainUpdate]

    def __init__(
        self,
        server_url: str,
        private_key: Keypair,
        bid_amount: int,
        chain_id: str,
        svm_rpc_endpoint: str,
        api_key: str | None = None,
    ):
        self.client = ExpressRelayClient(
            server_url,
            api_key,
            self.opportunity_callback,
            self.bid_status_callback,
            self.svm_chain_update_callback,
            self.remove_opportunities_callback,
        )
        self.private_key = private_key
        self.bid_amount = bid_amount
        self.chain_id = chain_id
        if self.chain_id not in SVM_CONFIGS:
            raise ValueError(f"Chain ID {self.chain_id} not supported")
        self.svm_config = SVM_CONFIGS[self.chain_id]
        self.rpc_client = AsyncClient(svm_rpc_endpoint)
        self.limo_client = LimoClient(self.rpc_client)
        self.express_relay_metadata = None
        self.mint_decimals_cache = {}
        self.latest_chain_update = {}

        self.logger = logging.getLogger("searcher")
        self.setup_logger()
        self.logger.info("Using searcher pubkey: %s", self.private_key.pubkey())

    def setup_logger(self):
        self.logger.setLevel(logging.INFO)
        log_handler = logging.StreamHandler()
        formatter = logging.Formatter(
            "%(asctime)s %(levelname)s:%(name)s:%(module)s %(message)s",
            datefmt="%Y-%m-%d %H:%M:%S",
        )
        log_handler.setFormatter(formatter)
        self.logger.addHandler(log_handler)

    async def opportunity_callback(self, opp: Opportunity):
        """
        Callback function to run when a new opportunity is found.

        Args:
            opp: An object representing a single opportunity.
        """
        if opp.chain_id not in self.latest_chain_update:
            self.logger.info(
                f"No recent blockhash for chain, {opp.chain_id} skipping bid"
            )
            return None

        bid = await self.generate_bid(typing.cast(OpportunitySvm, opp))

        if bid:
            try:
                bid_id = await self.client.submit_bid(bid)
                self.logger.info(
                    f"Submitted bid {str(bid_id)} for opportunity {str(opp.opportunity_id)}"
                )
            except Exception as e:
                self.logger.error(
                    f"Error submitting bid for opportunity {str(opp.opportunity_id)}: {e}"
                )

    async def bid_status_callback(self, bid_status_update: BidStatusUpdate):
        """
        Callback function to run when a bid status is updated.

        Args:
            bid_status_update: An object representing an update to the status of a bid.
        """
        id = bid_status_update.id
        status = bid_status_update.bid_status.type
        result = bid_status_update.bid_status.result

        result_details = ""
        if (
            status == BidStatusVariantsSvm.SUBMITTED
            or status == BidStatusVariantsSvm.WON
        ):
            result_details = f", transaction {result}"
        elif status == BidStatusVariantsSvm.LOST:
            if result:
                result_details = f", transaction {result}"
        self.logger.info(f"Bid status for bid {id}: {status.value}{result_details}")

    async def get_mint_decimals(self, mint: Pubkey) -> int:
        if str(mint) not in self.mint_decimals_cache:
            self.mint_decimals_cache[
                str(mint)
            ] = await self.limo_client.get_mint_decimals(mint)
        return self.mint_decimals_cache[str(mint)]

    async def generate_bid(self, opp: OpportunitySvm) -> BidSvm:
        if opp.program == "limo":
            return await self.generate_bid_limo(opp)
        if opp.program == "swap":
            return await self.generate_bid_swap(opp)

    async def generate_bid_limo(self, opp: LimoOpportunitySvm) -> OnChainBidSvm:
        """
        Generates a bid for a given opportunity.
        The transaction in this bid transfers assets from the searcher's wallet to fulfill the limit order.

        Args:
            opp: The SVM opportunity to bid on.
        Returns:
            The generated bid object.
        """
        order: OrderStateAndAddress = {"address": opp.order_address, "state": opp.order}

        ixs_take_order = await self.generate_take_order_ixs(order)
        bid_amount = await self.get_bid_amount(opp)
        router = self.limo_client.get_pda_authority(
            self.limo_client.get_program_id(), order["state"].global_config
        )

        submit_bid_ix = self.client.get_svm_submit_bid_instruction(
            searcher=self.private_key.pubkey(),
            router=router,
            permission_key=order["address"],
            bid_amount=bid_amount,
            deadline=DEADLINE,
            chain_id=self.chain_id,
            fee_receiver_relayer=(await self.get_metadata()).fee_receiver_relayer,
            relayer_signer=(await self.get_metadata()).relayer_signer,
        )
        latest_chain_update = self.latest_chain_update[self.chain_id]
        fee_instruction = set_compute_unit_price(
            latest_chain_update.latest_prioritization_fee
        )
        transaction = Transaction.new_with_payer(
            [fee_instruction, submit_bid_ix] + ixs_take_order, self.private_key.pubkey()
        )
        transaction.partial_sign(
            [self.private_key], recent_blockhash=latest_chain_update.blockhash
        )
        bid = OnChainBidSvm(
            transaction=transaction, chain_id=self.chain_id, slot=opp.slot
        )
        return bid

    async def generate_bid_swap(self, opp: SwapOpportunitySvm) -> SwapBidSvm:
        bid_amount = await self.get_bid_amount(opp)

        swap_ixs = self.client.get_svm_swap_instructions(
            searcher=self.private_key.pubkey(),
            bid_amount=bid_amount,
            deadline=DEADLINE,
            chain_id=self.chain_id,
            swap_opportunity=opp,
            relayer_signer=(await self.get_metadata()).relayer_signer,
        )
        latest_chain_update = self.latest_chain_update[self.chain_id]
        fee_instruction = set_compute_unit_price(
            latest_chain_update.latest_prioritization_fee
        )
        transaction = Transaction.new_with_payer(
            [fee_instruction] + swap_ixs, self.private_key.pubkey()
        )
        transaction.partial_sign(
            [self.private_key], recent_blockhash=latest_chain_update.blockhash
        )
        bid = SwapBidSvm(
            transaction=transaction,
            chain_id=self.chain_id,
            opportunity_id=opp.opportunity_id,
        )
        return bid

    async def generate_take_order_ixs(
        self, order: OrderStateAndAddress
    ) -> List[Instruction]:
        """
        Helper method to form the Limo instructions to take an order.

        Args:
            order: An object representing the order to be fulfilled.
        Returns:
            A list of Limo instructions to take an order.
        """
        input_amount = self.get_input_amount(order)
        output_amount = (
            order["state"].expected_output_amount * input_amount
            + order["state"].initial_input_amount
            - 1  # take the ceiling of the division by adding order[state].initial_input_amount - 1
        ) // order["state"].initial_input_amount

        input_mint_decimals = await self.get_mint_decimals(order["state"].input_mint)
        output_mint_decimals = await self.get_mint_decimals(order["state"].output_mint)
        self.logger.info(
            f"Order address {order['address']}\n"
            f"Fill rate {input_amount / order['state'].initial_input_amount}\n"
            f"Sell token {order['state'].input_mint} amount: {Decimal(input_amount) / Decimal(10 ** input_mint_decimals)}\n"
            f"Buy token {order['state'].output_mint} amount: {Decimal(output_amount) / Decimal(10**output_mint_decimals)}"
        )
        ixs_take_order = self.limo_client.take_order_ix(
            self.private_key.pubkey(),
            order,
            input_amount,
            output_amount,
            self.svm_config["express_relay_program"],
        )
        return ixs_take_order

    def get_input_amount(self, order: OrderStateAndAddress) -> int:
        return order["state"].remaining_input_amount

    async def get_metadata(self) -> ExpressRelayMetadata:
        if self.express_relay_metadata is None:
            self.express_relay_metadata = await ExpressRelayMetadata.fetch(
                self.rpc_client,
                self.limo_client.get_express_relay_metadata_pda(
                    SVM_EXPRESS_RELAY_PROGRAM_ID
                ),
                commitment=Finalized,
            )
            if self.express_relay_metadata is None:
                raise ValueError("Express relay metadata account not found")
        return self.express_relay_metadata

    async def get_bid_amount(self, opp: OpportunitySvm) -> int:
        """
        Args:
            opp: The SVM opportunity to bid on.
        Returns:
            The bid amount in the necessary token
        """

        return self.bid_amount

    async def svm_chain_update_callback(self, svm_chain_update: SvmChainUpdate):
        self.latest_chain_update[svm_chain_update.chain_id] = svm_chain_update

    # NOTE: Developers are responsible for implementing custom removal logic specific to their use case.
    async def remove_opportunities_callback(
        self, opportunity_delete: OpportunityDelete
    ):
        print(f"Opportunities {opportunity_delete} don't exist anymore")


def get_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser()
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument(
        "--private-key",
        type=str,
        help="Private key of the searcher in base58 format",
    )
    group.add_argument(
        "--private-key-json-file",
        type=str,
        help="Path to a json file containing the private key of the searcher in array of bytes format",
    )
    parser.add_argument(
        "--chain-id",
        type=str,
        required=True,
        choices=SVM_CONFIGS.keys(),
        help="Chain ID of the SVM network to submit bids",
    )
    parser.add_argument(
        "--endpoint-express-relay",
        type=str,
        required=True,
        help="Server endpoint to use for submitting bids",
    )
    parser.add_argument(
        "--endpoint-svm",
        type=str,
        required=True,
        help="Server endpoint to use for submitting bids",
    )
    parser.add_argument(
        "--api-key",
        type=str,
        required=False,
        help="The API key of the searcher to authenticate with the server for fetching and submitting bids",
    )
    parser.add_argument(
        "--bid",
        type=int,
        default=100,
        help="The amount of bid to submit for each opportunity",
    )
    return parser


async def main():
    parser = get_parser()
    args = parser.parse_args()

    if args.private_key:
        searcher_keypair = Keypair.from_base58_string(args.private_key)
    else:
        with open(args.private_key_json_file, "r") as f:
            searcher_keypair = Keypair.from_json(f.read())

    searcher = SimpleSearcherSvm(
        args.endpoint_express_relay,
        searcher_keypair,
        args.bid,
        args.chain_id,
        args.endpoint_svm,
        args.api_key,
    )

    await searcher.client.subscribe_chains([args.chain_id])

    task = await searcher.client.get_ws_loop()
    await task


if __name__ == "__main__":
    asyncio.run(main())
