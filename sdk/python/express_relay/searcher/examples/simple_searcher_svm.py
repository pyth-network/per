import argparse
import asyncio
import logging
import random
import typing
from typing import List
from decimal import Decimal

from solana.rpc.async_api import AsyncClient
from solana.rpc.commitment import Finalized
from solders.keypair import Keypair
from solders.pubkey import Pubkey
from solders.transaction import Transaction
from solders.instruction import Instruction

from express_relay.client import (
    ExpressRelayClient,
)
from express_relay.constants import SVM_CONFIGS
from express_relay.models import (
    BidStatusUpdate, Opportunity, OpportunityDelete
)
from express_relay.models.base import BidStatus
from express_relay.models.svm import BidSvm, OpportunitySvm, SvmChainUpdate, SvmHash
from express_relay.svm.generated.express_relay.accounts.express_relay_metadata import (
    ExpressRelayMetadata,
)
from express_relay.svm.generated.express_relay.program_id import (
    PROGRAM_ID as SVM_EXPRESS_RELAY_PROGRAM_ID,
)
from express_relay.svm.limo_client import LimoClient, OrderStateAndAddress

DEADLINE = 2**62
logger = logging.getLogger(__name__)

class BidData:
    def __init__(self, router: Pubkey, bid_amount: int, relayer_signer: Pubkey, relayer_fee_receiver: Pubkey):
        self.router = router
        self.bid_amount = bid_amount
        self.relayer_signer = relayer_signer
        self.relayer_fee_receiver = relayer_fee_receiver

class SimpleSearcherSvm:
    express_relay_metadata: ExpressRelayMetadata | None
    mint_decimals_cache: typing.Dict[str, int]
    recent_blockhash: typing.Dict[str, SvmHash]

    def __init__(
        self,
        server_url: str,
        private_key: Keypair,
        bid_amount: int,
        chain_id: str,
        svm_rpc_endpoint: str,
        fill_rate: int,
        with_latency: bool,
        bid_margin: int,
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
        self.fill_rate = fill_rate
        self.with_latency = with_latency
        self.bid_margin = bid_margin
        self.express_relay_metadata = None
        self.mint_decimals_cache = {}
        self.recent_blockhash = {}
    async def opportunity_callback(self, opp: Opportunity):
        """
        Callback function to run when a new opportunity is found.

        Args:
            opp: An object representing a single opportunity.
        """
        if opp.chain_id not in self.recent_blockhash:
            logger.info(f"No recent blockhash for chain, {opp.chain_id} skipping bid")
            return None

        if self.with_latency:
            await asyncio.sleep(0.5 * random.random())

        bid = await self.assess_opportunity(typing.cast(OpportunitySvm, opp))

        if bid:
            try:
                bid_id = await self.client.submit_bid(bid)
                logger.info(f"Submitted bid {str(bid_id)} for opportunity {str(opp.opportunity_id)}")
            except Exception as e:
                logger.error(
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
        if status == BidStatus.SUBMITTED or status == BidStatus.WON:
            result_details = f", transaction {result}"
        elif status == BidStatus.LOST:
            if result:
                result_details = f", transaction {result}"
        logger.info(f"Bid status for bid {id}: {status.value}{result_details}")

    async def get_mint_decimals(self, mint: Pubkey) -> int:
        if str(mint) not in self.mint_decimals_cache:
            self.mint_decimals_cache[
                str(mint)
            ] = await self.limo_client.get_mint_decimals(mint)
        return self.mint_decimals_cache[str(mint)]

    async def assess_opportunity(self, opp: OpportunitySvm) -> BidSvm | None:
        """
        Method to assess an opportunity and return a bid if the opportunity is worth taking. This method always returns a bid for any valid opportunity. The transaction in this bid transfers assets from the searcher's wallet to fulfill the limit order.

        Args:
            opp: An object representing a single opportunity.
        Returns:
            A bid object if the opportunity is worth taking to be submitted to the Express Relay server, otherwise None.
        """
        order: OrderStateAndAddress = {"address": opp.order_address, "state": opp.order}
        ixs_take_order = await self.generate_take_order_ixs(order)
        bid_data = await self.get_bid_data(order)

        submit_bid_ix = self.client.get_svm_submit_bid_instruction(
            searcher=self.private_key.pubkey(),
            router=bid_data.router,
            permission_key=order["address"],
            bid_amount=bid_data.bid_amount,
            deadline=DEADLINE,
            chain_id=self.chain_id,
            fee_receiver_relayer=bid_data.relayer_fee_receiver,
            relayer_signer=bid_data.relayer_signer,
        )
        transaction = Transaction.new_with_payer(
            [submit_bid_ix] + ixs_take_order, self.private_key.pubkey()
        )
        transaction.partial_sign(
            [self.private_key], recent_blockhash=self.recent_blockhash[self.chain_id]
        )
        bid = BidSvm(transaction=transaction, chain_id=self.chain_id)
        return bid

    async def generate_take_order_ixs(self, order: OrderStateAndAddress) -> List[Instruction]:
        """
        Helper method to form the Limo instructions to take an order.

        Args:
            order: An object representing the order to be fulfilled.
        Returns:
            A list of Limo instructions to take an order.
        """
        input_mint_decimals = await self.get_mint_decimals(order["state"].input_mint)
        output_mint_decimals = await self.get_mint_decimals(order["state"].output_mint)
        effective_fill_rate = min(
            self.fill_rate,
            100
            * order["state"].remaining_input_amount
            / order["state"].initial_input_amount,
        )
        input_amount_decimals = Decimal(order["state"].initial_input_amount) / Decimal(
            10**input_mint_decimals
        )
        input_amount_decimals = (
            input_amount_decimals * Decimal(effective_fill_rate) / Decimal(100)
        )
        output_amount_decimals = Decimal(
            order["state"].expected_output_amount
        ) / Decimal(10**output_mint_decimals)
        logger.info(
            f"Order address {order['address']}\n"
            f"Sell token {order['state'].input_mint} amount: {input_amount_decimals}\n"
            f"Buy token {order['state'].output_mint} amount: {output_amount_decimals}"
        )
        ixs_take_order = await self.limo_client.take_order_ix(
            self.private_key.pubkey(),
            order,
            input_amount_decimals,
            output_amount_decimals,
            input_mint_decimals,
            output_mint_decimals,
            self.svm_config["express_relay_program"],
        )
        return ixs_take_order

    async def get_bid_data(self, order: OrderStateAndAddress) -> BidData:
        """
        Helper method to get the bid data for an opportunity.

        Args:
            order: An object representing the order to be fulfilled.
        Returns:
            A BidData object representing the bid data for the opportunity. Consists of the router pubkey, bid amount, relayer signer pubkey, and relayer fee receiver pubkey.
        """
        router = self.limo_client.get_pda_authority(
            self.limo_client.get_program_id(), order["state"].global_config
        )

        bid_amount = self.bid_amount
        if self.bid_margin != 0:
            bid_amount += random.randint(-self.bid_margin, self.bid_margin)

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

        return BidData(
            router=router,
            bid_amount=bid_amount,
            relayer_signer=self.express_relay_metadata.relayer_signer,
            relayer_fee_receiver=self.express_relay_metadata.fee_receiver_relayer
        )

    async def svm_chain_update_callback(self, svm_chain_update: SvmChainUpdate):
        self.recent_blockhash[svm_chain_update.chain_id] = svm_chain_update.blockhash

    # NOTE: Developers are responsible for implementing custom removal logic specific to their use case.
    async def remove_opportunities_callback(self, opportunity_delete: OpportunityDelete):
        print(f"Opportunities {opportunity_delete} don't exist anymore")


async def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("-v", "--verbose", action="count", default=0)
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
    parser.add_argument(
        "--fill-rate",
        type=int,
        default=100,
        help="How much of the initial order size to fill in percentage. Default is 100%",
    )
    parser.add_argument(
        "--with-latency",
        required=False,
        default=False,
        action="store_true",
        help="Whether to add random latency to the bid submission. Default is false",
    )
    parser.add_argument(
        "--bid-margin",
        required=False,
        type=int,
        default=0,
        help="The margin to add or subtract from the bid. For example, 1 means the bid range is [bid - 1, bid + 1]. Default is 0",
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

    if args.private_key:
        searcher_keypair = Keypair.from_base58_string(args.private_key)
    else:
        with open(args.private_key_json_file, "r") as f:
            searcher_keypair = Keypair.from_json(f.read())

    logger.info("Using Keypair with pubkey: %s", searcher_keypair.pubkey())
    searcher = SimpleSearcherSvm(
        args.endpoint_express_relay,
        searcher_keypair,
        args.bid,
        args.chain_id,
        args.endpoint_svm,
        args.fill_rate,
        args.with_latency,
        args.bid_margin,
        args.api_key,
    )

    await searcher.client.subscribe_chains([args.chain_id])

    task = await searcher.client.get_ws_loop()
    await task


if __name__ == "__main__":
    asyncio.run(main())
