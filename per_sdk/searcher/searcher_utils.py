from typing import TypedDict

import web3
from eth_account import Account
from eth_account.datastructures import SignedMessage

from per_sdk.utils.types_liquidation_adapter import OpportunitySignatureConfig

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
    signature_config: OpportunitySignatureConfig,
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
        signing_data: The extra data you need to sign the opportunity.
    Returns:
        An EIP712 SignedMessage object, representing the liquidator's signature.
    """

    executor = Account.from_key(secret_key).address
    domain_data = {
        "name": signature_config["domain_name"],
        "version": signature_config["domain_version"],
        "chainId": signature_config["chain_network_id"],
        "verifyingContract": signature_config["contract_address"],
    }
    message_types = {
        "Signature": [
            {"name": "executionParams", "type": "ExecutionParams"},
            {"name": "signer", "type": "address"},
            {"name": "deadline", "type": "uint256"},
        ],
        "ExecutionParams": [
            {"name": "sellTokens", "type": "TokenAmount[]"},
            {"name": "buyTokens", "type": "TokenAmount[]"},
            {"name": "targetContract", "type": "address"},
            {"name": "targetCalldata", "type": "bytes"},
            {"name": "targetCallValue", "type": "uint256"},
            {"name": "bidAmount", "type": "uint256"},
        ],
        "TokenAmount": [
            {"name": "token", "type": "address"},
            {"name": "amount", "type": "uint256"},
        ],
    }

    # the data to be signed
    message_data = {
        "executionParams": {
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
            "targetContract": address,
            "targetCalldata": calldata,
            "targetCallValue": value,
            "bidAmount": bid_info["bid"],
        },
        "signer": executor,
        "deadline": bid_info["valid_until"],
    }

    signed_typed_data = Account.sign_typed_data(
        secret_key, domain_data, message_types, message_data
    )
    return signed_typed_data
