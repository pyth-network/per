from typing import TypedDict

import web3
from eth_abi import encode
from web3.auto import w3


class UserLiquidationParams(TypedDict):
    bid: int
    valid_until: int


def construct_signature_liquidator(
    repay_tokens: list[(str, int)],
    receipt_tokens: list[(str, int)],
    address: str,
    liq_calldata: bytes,
    value: int,
    bid: int,
    valid_until: int,
    secret_key: str,
):
    """
    Constructs a signature for a liquidator's transaction to submit to the LiquidationAdapter contract.

    Args:
        repay_tokens: A list of tuples (token address, amount) representing the tokens to repay.
        receipt_tokens: A list of tuples (token address, amount) representing the tokens to receive.
        address: The address of the LiquidationAdapter contract.
        liq_calldata: The calldata for the liquidation method call.
        value: The value for the liquidation method call.
        bid: The amount of native token to bid on this opportunity.
        valid_until: The timestamp at which the transaction will expire.
        secret_key: A 0x-prefixed hex string representing the liquidator's private key.
    Returns:
        An web3 ECDSASignature object, representing the liquidator's signature.
    """

    digest = encode(
        [
            "(address,uint256)[]",
            "(address,uint256)[]",
            "address",
            "bytes",
            "uint256",
            "uint256",
        ],
        [repay_tokens, receipt_tokens, address, liq_calldata, value, bid],
    )
    msg_data = web3.Web3.solidity_keccak(["bytes", "uint256"], [digest, valid_until])
    signature = w3.eth.account.signHash(msg_data, private_key=secret_key)

    return signature
