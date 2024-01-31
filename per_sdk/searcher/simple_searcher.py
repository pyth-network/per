import argparse
import asyncio
import logging
from typing import TypedDict

import httpx
from eth_account import Account

from per_sdk.searcher.searcher_utils import (
    UserLiquidationParams,
    construct_signature_liquidator,
)
from per_sdk.utils.endpoints import (
    LIQUIDATION_SERVER_ENDPOINT_BID,
    LIQUIDATION_SERVER_ENDPOINT_GETOPPS,
)
from per_sdk.utils.types_liquidation_adapter import LiquidationOpportunity

logger = logging.getLogger(__name__)

VALID_UNTIL = 1_000_000_000_000


def assess_liquidation_opportunity(
    default_bid: int,
    opp: LiquidationOpportunity,
) -> UserLiquidationParams | None:
    user_liquidation_params: UserLiquidationParams = {
        "bid": default_bid,
        "valid_until": VALID_UNTIL,
    }
    return user_liquidation_params


class OpportunityBid(TypedDict):
    opportunity_id: str
    permission_key: str
    bid_amount: str
    valid_until: str
    liquidator: str
    signature: str


def create_liquidation_transaction(
    opp: LiquidationOpportunity, sk_liquidator: str, valid_until: int, bid: int
) -> OpportunityBid:
    repay_tokens = [
        (opp["contract"], int(opp["amount"])) for opp in opp["repay_tokens"]
    ]
    receipt_tokens = [
        (opp["contract"], int(opp["amount"])) for opp in opp["receipt_tokens"]
    ]

    liquidator = Account.from_key(sk_liquidator).address
    liq_calldata = bytes.fromhex(opp["calldata"].replace("0x", ""))

    signature_liquidator = construct_signature_liquidator(
        repay_tokens,
        receipt_tokens,
        opp["contract"],
        liq_calldata,
        int(opp["value"]),
        bid,
        valid_until,
        sk_liquidator,
    )

    json_body = {
        "chain_id": opp["chain_id"],
        "opportunity_id": opp["opportunity_id"],
        "permission_key": opp["permission_key"],
        "bid_amount": str(bid),
        "valid_until": str(valid_until),
        "liquidator": liquidator,
        "signature": bytes(signature_liquidator.signature).hex(),
    }

    return json_body


async def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("-v", "--verbose", action="count", default=0)
    parser.add_argument(
        "--private-key",
        type=str,
        required=True,
        help="Private key of the searcher for signing calldata",
    )
    parser.add_argument(
        "--chain-id",
        type=str,
        required=True,
        help="Chain ID of the network to monitor for liquidation opportunities",
    )
    parser.add_argument(
        "--bid",
        type=int,
        default=10,
        help="Default amount of bid for liquidation opportunities",
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

    params = {"chain_id": args.chain_id}
    sk_liquidator = args.private_key
    liquidator = Account.from_key(sk_liquidator).address
    logger.info("Liquidator address: %s", liquidator)
    client = httpx.AsyncClient()
    while True:
        try:
            accounts_liquidatable = (
                await client.get(LIQUIDATION_SERVER_ENDPOINT_GETOPPS, params=params)
            ).json()
        except Exception as e:
            logger.error(e)
            await asyncio.sleep(5)
            continue

        logger.debug("Found %d liquidation opportunities", len(accounts_liquidatable))
        for liquidation_opp in accounts_liquidatable:
            user_liquidation_params = assess_liquidation_opportunity(
                args.bid, liquidation_opp
            )

            if user_liquidation_params is not None:
                bid, valid_until = (
                    user_liquidation_params["bid"],
                    user_liquidation_params["valid_until"],
                )

                tx = create_liquidation_transaction(
                    liquidation_opp, sk_liquidator, valid_until, bid
                )

                resp = await client.post(LIQUIDATION_SERVER_ENDPOINT_BID, json=tx)
                logger.info(
                    "Submitted bid amount %s for opportunity %s, server response: %s",
                    bid,
                    liquidation_opp["opportunity_id"],
                    resp.text,
                )

        await asyncio.sleep(1)


if __name__ == "__main__":
    asyncio.run(main())
