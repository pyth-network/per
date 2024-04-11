from typing import TypedDict

import web3
from eth_abi import encode
from eth_account import Account
from eth_account.datastructures import SignedMessage
from web3.auto import w3

from per_sdk.utils.types_liquidation_adapter import OpportunitySignatureConfig

solidity_keccak = web3.Web3.solidity_keccak


class BidInfo(TypedDict):
    bid: int
    valid_until: int


class EIP712:
    def __init__(
        self, _name: str, _version: str, _chain_id: int, _verifying_contract: str
    ):
        self.name = _name
        self.version = _version
        self.chain_id = _chain_id
        self.verifying_contract = _verifying_contract

    def _name_hash(self) -> bytes:
        return solidity_keccak(["string"], [self.name])

    def _version_hash(self) -> bytes:
        return solidity_keccak(["string"], [self.version])

    def _type_hash(self) -> bytes:
        return solidity_keccak(
            ["string"],
            [
                "EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)"
            ],
        )

    def domain_seperator_v4(self):
        encoded = encode(
            ["bytes32", "bytes32", "bytes32", "uint256", "address"],
            [
                self._type_hash(),
                self._name_hash(),
                self._version_hash(),
                self.chain_id,
                self.verifying_contract,
            ],
        )
        domain_hash = solidity_keccak(["bytes"], [encoded])
        return domain_hash

    @staticmethod
    def hash_typed_data_v4(domain_seperator: bytes, data: bytes) -> bytes:
        prefix = b"\x19\x01"
        return web3.Web3.solidity_keccak(
            ["bytes", "bytes", "bytes"], [prefix, domain_seperator, data]
        )


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

    digest = encode(
        [
            "(address,uint256)[]",
            "(address,uint256)[]",
            "address",
            "bytes",
            "uint256",
            "uint256",
            "uint256",
        ],
        [
            sell_tokens,
            buy_tokens,
            address,
            calldata,
            value,
            bid_info["bid"],
            bid_info["valid_until"],
        ],
    )

    data_digest = encode(
        ["bytes32", "address", "bytes", "uint256"],
        [
            solidity_keccak(["string"], [signature_config["opportunity_type"]]),
            Account.from_key(secret_key).address,
            digest,
            bid_info["valid_until"],
        ],
    )
    data_hash = solidity_keccak(["bytes"], [data_digest])

    eip712 = EIP712(
        signature_config["domain_name"],
        signature_config["domain_version"],
        signature_config["chain_network_id"],
        signature_config["contract_address"],
    )
    structured_data_hash = EIP712.hash_typed_data_v4(
        eip712.domain_seperator_v4(), data_hash
    )

    signature = w3.eth.account.signHash(structured_data_hash, private_key=secret_key)
    return signature
