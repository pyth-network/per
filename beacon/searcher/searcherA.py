import web3
from web3.auto import w3
from eth_account import Account
from eth_account.signers.local import LocalAccount
from eth_abi import encode
import httpx
import asyncio

from beacon.utils.types_liquidation_adapter import *
from beacon.utils.endpoints import *
from beacon.searcher.searcher_utils import *

BID = 10
VALID_UNTIL = 1_000_000_000_000

class UserLiquidationParams(TypedDict):
    bid: int
    valid_until: int

def assess_liquidation_opportunity(
    opp: LiquidationOpportunity
) -> UserLiquidationParams | None:
    user_liquidation_params: UserLiquidationParams = {
        "bid": BID,
        "valid_until": VALID_UNTIL
    }
    return user_liquidation_params

def create_liquidation_transaction(
    opp: LiquidationOpportunity,
    sk_liquidator: str,
    valid_until: int,
    bid: int
) -> LiquidationAdapterTransaction:
    repay_tokens = [(opp['repay_tokens'][0][0],
                     int(opp['repay_tokens'][0][1], 16))]
    receipt_tokens = [(opp['receipt_tokens'][0][0],
                       int(opp['receipt_tokens'][0][1], 16))]

    account: LocalAccount = Account.from_key(sk_liquidator)
    liquidator = account.address
    liq_calldata = bytes.fromhex(
        opp['calldata'][2:]) if opp['calldata'][:2] == "0x" else bytes.fromhex(opp['calldata'])

    signature_liquidator = construct_signature_liquidator(
        repay_tokens, receipt_tokens, opp['contract'], liq_calldata, bid, valid_until, sk_liquidator)

    liquidation_adapter_calldata: LiquidationAdapterCalldata = {
        "repay_tokens": repay_tokens,
        "expected_receipt_tokens": receipt_tokens,
        "liquidator": liquidator,
        "contract": opp['contract'],
        "data": liq_calldata,
        "valid_until": valid_until,
        "bid": bid,
        "signature_liquidator": bytes(signature_liquidator.signature)
    }
    calldata = LIQUIDATION_ADAPTER_FN_SIGNATURE + \
        encode([LIQUIDATION_ADAPTER_CALLDATA_TYPES], [
               tuple(liquidation_adapter_calldata.values())]).hex()

    tx: LiquidationAdapterTransaction = {
        "bid": hex(bid),
        "calldata": calldata,
        "chain_id": opp["chain_id"],
        "contract": LIQUIDATION_ADAPTER_ADDRESS,
        "permission_key": opp['permission_key']
    }

    return tx


async def main():
    client = httpx.AsyncClient()

    params = {"chain_id": "development"}

    # this is hardcoded to the searcher A SK
    sk_liquidator = "0x5b1efe5da513271c0d30cde7a2ad1d29456d68abd592efdaa7d2302e913b783f"

    while True:
        liquidatable = (await client.get(BEACON_SERVER_ENDPOINT_GETOPPS, params=params)).json()

        for liquidation_opp in liquidatable:
            user_liquidation_params = assess_liquidation_opportunity(liquidation_opp)

            if user_liquidation_params is not None:
                bid, valid_until = user_liquidation_params["bid"], user_liquidation_params["valid_until"]

                tx = create_liquidation_transaction(
                    liquidation_opp, sk_liquidator, valid_until, bid)

                resp = await client.post(
                    AUCTION_SERVER_ENDPOINT,
                    json=tx
                )

                print(resp.text)

                import pdb
                pdb.set_trace()

if __name__ == "__main__":
    asyncio.run(main())
