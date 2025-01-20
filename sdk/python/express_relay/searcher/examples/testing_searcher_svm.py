import asyncio
import random

from express_relay.models import Opportunity
from express_relay.svm.limo_client import OrderStateAndAddress
from solders.keypair import Keypair

from .simple_searcher_svm import SimpleSearcherSvm, get_parser


class TestingSearcherSvm(SimpleSearcherSvm):
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
        super().__init__(
            server_url, private_key, bid_amount, chain_id, svm_rpc_endpoint, api_key
        )
        self.fill_rate = fill_rate
        self.with_latency = with_latency
        self.bid_margin = bid_margin

    async def opportunity_callback(self, opp: Opportunity):
        if self.with_latency:
            latency = 0.5 * random.random()
            self.logger.info(f"Adding latency of {latency * 100}ms")
            await asyncio.sleep(latency)
        return await super().opportunity_callback(opp)

    async def get_bid_amount(self, order: OrderStateAndAddress) -> int:
        return self.bid_amount + random.randint(-self.bid_margin, self.bid_margin)

    def get_input_amount(self, order: OrderStateAndAddress) -> int:
        return min(
            super().get_input_amount(order),
            order["state"].initial_input_amount * self.fill_rate // 100,
        )


async def main():
    parser = get_parser()
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

    if args.private_key:
        searcher_keypair = Keypair.from_base58_string(args.private_key)
    else:
        with open(args.private_key_json_file, "r") as private_key_file:
            searcher_keypair = Keypair.from_json(private_key_file.read())

    searcher = TestingSearcherSvm(
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
