from typing import TypedDict


class TokenAmount(TypedDict):
    contract: str
    amount: str


class EIP712Domain(TypedDict):
    # The name parameter for the EIP712 domain separator.
    name: str | None
    # The version parameter for the EIP712 domain separator.
    verion: str | None
    # The network chain id parameter for the EIP712 domain separator.
    chain_id: str | None
    # The verifying contract parameter for the EIP712 domain separator.
    verifying_contract: str | None


class Opportunity(TypedDict):
    # The unique id of the opportunity
    opportunity_id: str
    # The id of the chain where the opportunity was found
    chain_id: str
    # Address of the contract where the liquidation method is called
    target_contract: str
    # The calldata that needs to be passed in with the liquidation method call
    target_calldata: str
    # The value that needs to be passed in with the liquidation method call
    target_call_value: str
    # The permission key necessary to call the liquidation method
    permission_key: str
    # A list of tokens that can be used to repay this account's debt.
    sell_tokens: list[TokenAmount]
    # A list of tokens that ought to be received by the liquidator in exchange for the sell tokens.
    buy_tokens: list[TokenAmount]
    # The eip712 domain config to be used for signing the opportunity
    eip_712_domain: EIP712Domain
    # Opportunity format version, used to determine how to interpret the opportunity data
    version: str


class OpportunityAdapterCalldata(TypedDict):
    sell_tokens: list[(str, int)]
    buy_tokens: list[(str, int)]
    liquidator: str
    contract: str
    data: bytes
    valid_until: int
    bid: int
    signature_liquidator: bytes


class OpportunityAdapterTransaction(TypedDict):
    bid: str
    calldata: str
    chain_id: str
    contract: str
    permission_key: str
