from typing import TypedDict

from eth_account import Account
from eth_account.datastructures import SignedMessage


class BidInfo(TypedDict):
    bid: int
    deadline: int
    nonce: int


def _get_permitted_tokens(
    sell_tokens: list[(str, int)], bid_amount: int, call_value: int, weth_address: str
) -> list[dict[str, int]]:
    permitted_tokens = [
        {
            "token": token[0],
            "amount": int(token[1]),
        }
        for token in sell_tokens
    ]

    for token in permitted_tokens:
        if token["token"] == weth_address:
            token["amount"] += call_value + bid_amount
            return permitted_tokens

    if bid_amount + call_value > 0:
        permitted_tokens.append(
            {
                "token": weth_address,
                "amount": bid_amount + call_value,
            }
        )

    return permitted_tokens


# Reference https://eips.ethereum.org/EIPS/eip-712
def construct_signature_executor(
    sell_tokens: list[(str, int)],
    buy_tokens: list[(str, int)],
    address: str,
    calldata: bytes,
    value: int,
    bid_info: BidInfo,
    secret_key: str,
    opportunity_adapter_address: str,
    weth_address: str,
    permit2_address: str,
    chain_id: int,
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
        deadline: The timestamp at which the transaction will expire.
        secret_key: A 0x-prefixed hex string representing the liquidator's private key.
        eip_712_domain: The EIP712 domain data to create the signature.
    Returns:
        An EIP712 SignedMessage object, representing the liquidator's signature.
    """
    executor = Account.from_key(secret_key).address
    domain_data = {
        "name": "Permit2",
        "chainId": str(chain_id),
        "verifyingContract": permit2_address,
    }

    message_types = {
        "PermitBatchWitnessTransferFrom": [
            {"name": "permitted", "type": "TokenPermissions[]"},
            {"name": "spender", "type": "address"},
            {"name": "nonce", "type": "uint256"},
            {"name": "deadline", "type": "uint256"},
            {"name": "witness", "type": "OpportunityWitness"},
        ],
        "OpportunityWitness": [
            {"name": "buyTokens", "type": "TokenAmount[]"},
            {"name": "targetCalldata", "type": "bytes"},
            {"name": "targetCallValue", "type": "uint256"},
            {"name": "targetContract", "type": "address"},
            {"name": "executor", "type": "address"},
            {"name": "bidAmount", "type": "uint256"},
        ],
        "TokenAmount": [
            {"name": "token", "type": "address"},
            {"name": "amount", "type": "uint256"},
        ],
        "TokenPermissions": [
            {"name": "token", "type": "address"},
            {"name": "amount", "type": "uint256"},
        ],
    }

    # the data to be signed
    message_data = {
        "permitted": _get_permitted_tokens(
            sell_tokens, bid_info["bid"], value, weth_address
        ),
        "spender": opportunity_adapter_address,
        "nonce": bid_info["nonce"],
        "deadline": bid_info["deadline"],
        "witness": {
            "buyTokens": [
                {
                    "token": token[0],
                    "amount": int(token[1]),
                }
                for token in buy_tokens
            ],
            "targetCalldata": calldata,
            "targetCallValue": value,
            "targetContract": address,
            "executor": executor,
            "bidAmount": bid_info["bid"],
        },
    }

    signed_typed_data = Account.sign_typed_data(
        secret_key, domain_data, message_types, message_data
    )
    return signed_typed_data
