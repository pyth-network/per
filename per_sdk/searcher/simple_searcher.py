import argparse
import asyncio
import logging
from typing import TypedDict

import httpx
from beacon.searcher.searcher_utils import (
    UserLiquidationParams,
    construct_signature_liquidator,
)
from beacon.utils.endpoints import (
    BEACON_SERVER_ENDPOINT_BID,
    BEACON_SERVER_ENDPOINT_GETOPPS,
)
from beacon.utils.types_liquidation_adapter import LiquidationOpportunity
from eth_account import Account
from eth_account.signers.local import LocalAccount

BID = 10
VALID_UNTIL = 1_000_000_000_000


def assess_liquidation_opportunity(
    opp: LiquidationOpportunity,
) -> UserLiquidationParams | None:
    user_liquidation_params: UserLiquidationParams = {
        "bid": BID,
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

    account: LocalAccount = Account.from_key(sk_liquidator)
    liquidator = account.address
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
    args = parser.parse_args()

    logging.basicConfig(level=logging.INFO)
    logging.getLogger("httpx").propagate = False

    params = {"chain_id": args.chain_id}
    sk_liquidator = args.private_key
    client = httpx.AsyncClient()
    while True:
        accounts_liquidatable = (
            await client.get(BEACON_SERVER_ENDPOINT_GETOPPS, params=params)
        ).json()

        for liquidation_opp in accounts_liquidatable:
            user_liquidation_params = assess_liquidation_opportunity(liquidation_opp)

            if user_liquidation_params is not None:
                bid, valid_until = (
                    user_liquidation_params["bid"],
                    user_liquidation_params["valid_until"],
                )

                tx = create_liquidation_transaction(
                    liquidation_opp, sk_liquidator, valid_until, bid
                )

                resp = await client.post(BEACON_SERVER_ENDPOINT_BID, json=tx)

                print(resp.text)
        await asyncio.sleep(5)


if __name__ == "__main__":
    asyncio.run(main())
