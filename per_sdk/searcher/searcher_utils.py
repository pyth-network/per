from typing import TypedDict

import web3
from eth_account import Account
from eth_account.datastructures import SignedMessage

from per_sdk.utils.types_liquidation_adapter import EIP712Domain

solidity_keccak = web3.Web3.solidity_keccak


class BidInfo(TypedDict):
    bid: int
    valid_until: int


# Reference https://eips.ethereum.org/EIPS/eip-712
def construct_signature_executor(
    sell_tokens: list[(str, int)],
    buy_tokens: list[(str, int)],
    address: str,
    calldata: bytes,
    value: int,
    bid_info: BidInfo,
    secret_key: str,
    eip_712_domain: EIP712Domain,
) -> SignedMessage:
    """
    Constructs a signature for an executors' bid to submit to the auction server.

    Args:
        sell_tokens: A list of tuples (token address, amount) representing the tokens to repay.
        buy_tokens: A list of tuples (token address, amount) representing the tokens to receive.
        address: The address of the protocol contract for the liquidation.
        calldata: The calldata for the execution method call.
        value: The value for the liquidation method call.
        bid: The amount of native token to bid on this opportunity.
        valid_until: The timestamp at which the transaction will expire.
        secret_key: A 0x-prefixed hex string representing the liquidator's private key.
        eip_712_domain: The EIP712 domain data to create the signature.
    Returns:
        An EIP712 SignedMessage object, representing the liquidator's signature.
    """

    executor = Account.from_key(secret_key).address
    domain_data = {
        "name": eip_712_domain["name"],
        "version": eip_712_domain["version"],
        "chainId": eip_712_domain["chain_id"],
        "verifyingContract": eip_712_domain["verifying_contract"],
    }
    message_types = {
        "ExecutionParams": [
            {"name": "sellTokens", "type": "TokenAmount[]"},
            {"name": "buyTokens", "type": "TokenAmount[]"},
            {"name": "executor", "type": "address"},
            {"name": "targetContract", "type": "address"},
            {"name": "targetCalldata", "type": "bytes"},
            {"name": "targetCallValue", "type": "uint256"},
            {"name": "validUntil", "type": "uint256"},
            {"name": "bidAmount", "type": "uint256"},
        ],
        "TokenAmount": [
            {"name": "token", "type": "address"},
            {"name": "amount", "type": "uint256"},
        ],
    }

    # the data to be signed
    message_data = {
        "sellTokens": [
            {
                "token": token[0],
                "amount": int(token[1]),
            }
            for token in sell_tokens
        ],
        "buyTokens": [
            {
                "token": token[0],
                "amount": int(token[1]),
            }
            for token in buy_tokens
        ],
        "executor": executor,
        "targetContract": address,
        "targetCalldata": calldata,
        "targetCallValue": value,
        "validUntil": bid_info["valid_until"],
        "bidAmount": bid_info["bid"],
    }

    signed_typed_data = Account.sign_typed_data(
        secret_key, domain_data, message_types, message_data
    )
    return signed_typed_data
