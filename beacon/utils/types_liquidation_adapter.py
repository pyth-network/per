from typing import TypedDict
import web3

from pythresearch.per.beacon.utils.pyth_prices import PriceFeed

LIQUIDATION_ADAPTER_ADDRESS = "0x2279B7A0a67DB372996a5FaB50D91eAA73d2eBe6"

class LiquidationOpportunity(TypedDict):
    chain_id: str
    # Address of the contract where the liquidation method is called
    contract: str
    # The calldata that needs to be passed in with the liquidation method call
    calldata: str
    permission_key: str
    account: str
    # A list of tokens that can be used to repay this account's debt. Each entry in the list is a tuple (token address, hex string of repay amount)
    repay_tokens: list[(str, str)]
    # A list of tokens that ought to be received by the liquidator in exchange for the repay tokens. Each entry in the list is a tuple (token address, hex string of receipt amount)
    receipt_tokens: list[(str, str)]
    prices: list[PriceFeed]



LIQUIDATION_ADAPTER_CALLDATA_TYPES = '((address,uint256)[],(address,uint256)[],address,address,bytes,uint256,uint256,bytes)'
LIQUIDATION_ADAPTER_FN_SIGNATURE = web3.Web3.solidity_keccak(["string"], [f"callLiquidation({LIQUIDATION_ADAPTER_CALLDATA_TYPES})"])[:4].hex()

class LiquidationAdapterCalldata(TypedDict):
    repay_tokens: list[(str, int)]
    expected_receipt_tokens: list[(str, int)]
    liquidator: str
    contract: str
    data: bytes
    valid_until: int
    bid: int
    signature_liquidator: bytes

class LiquidationAdapterIntent(TypedDict):
    bid: str
    calldata: str
    chain_id: str
    contract: str
    permission_key: str