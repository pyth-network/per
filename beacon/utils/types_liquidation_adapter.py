from typing import TypedDict

from pythresearch.per.beacon.utils.pyth_prices import PriceFeed

LIQUIDATION_ADAPTER_ADDRESS = "0x2279B7A0a67DB372996a5FaB50D91eAA73d2eBe6"

class LiquidationOpportunity(TypedDict):
    contract: str
    data: str
    permission: str | None
    account: (int | str)
    repay_tokens: list[(str, int, int)]
    receipt_tokens: list[(str, int, int)]
    prices: list[PriceFeed]


class LiquidationAdapterIntent(TypedDict):
    bid: str
    calldata: str
    chain_id: str
    contract: str
    permission_key: str