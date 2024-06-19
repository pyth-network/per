from typing import TypedDict

from eth_account import Account
from eth_account.datastructures import SignedMessage

from per_sdk.utils.types_liquidation_adapter import EIP712Domain


class BidInfo(TypedDict):
    bid: int
    valid_until: int
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
    eip_712_domain: EIP712Domain,
    opportunity_adapter_address: str,
    weth_address: str,
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
    domain_data = {}
    if eip_712_domain.get("name"):
        domain_data["name"] = eip_712_domain["name"]
    if eip_712_domain.get("version"):
        domain_data["version"] = eip_712_domain["version"]
    if eip_712_domain.get("chain_id"):
        domain_data["chainId"] = eip_712_domain["chain_id"]
    if eip_712_domain.get("verifying_contract"):
        domain_data["verifyingContract"] = eip_712_domain["verifying_contract"]

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
            {"name": "executor", "type": "address"},
            {"name": "targetContract", "type": "address"},
            {"name": "targetCalldata", "type": "bytes"},
            {"name": "targetCallValue", "type": "uint256"},
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
        "deadline": bid_info["valid_until"],
        "witness": {
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
            "bidAmount": bid_info["bid"],
        },
    }

    signed_typed_data = Account.sign_typed_data(
        secret_key, domain_data, message_types, message_data
    )
    return signed_typed_data
