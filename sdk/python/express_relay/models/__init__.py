from typing import Any, Union

from express_relay.models.base import IntString, UUIDString
from express_relay.models.evm import (
    Address,
    BidEvm,
    BidResponseEvm,
    BidStatusEvm,
    Bytes32,
    HexString,
    OpportunityDeleteEvm,
    OpportunityEvm,
    SignedMessageString,
    TokenAmount,
)
from express_relay.models.svm import (
    BidResponseSvm,
    BidStatusSvm,
    BidSvm,
    OpportunityDeleteSvm,
    OpportunitySvm,
    SvmTransaction,
)
from pydantic import BaseModel, Discriminator, Field, RootModel, Tag
from typing_extensions import Annotated, Literal

Bid = Union[BidEvm, BidSvm]


class BidStatusUpdate(BaseModel):
    """
    Attributes:
        id: The ID of the bid.
        bid_status: The current status of the bid.
    """

    id: UUIDString
    bid_status: Union[BidStatusEvm, BidStatusSvm]


BidResponse = Union[BidResponseEvm, BidResponseSvm]
BidResponseRoot = RootModel[BidResponse]


class OpportunityBidParams(BaseModel):
    """
    Attributes:
        amount: The amount of the bid in wei.
        nonce: The nonce of the bid.
        deadline: The unix timestamp after which the bid becomes invalid.
    """

    amount: IntString
    nonce: IntString
    deadline: IntString


class OpportunityBid(BaseModel):
    """
    Attributes:
        opportunity_id: The ID of the opportunity.
        amount: The amount of the bid in wei.
        executor: The address of the executor.
        permission_key: The permission key to bid on.
        signature: The signature of the bid.
        deadline: The unix timestamp after which the bid becomes invalid.
        nonce: The nonce of the bid.
    """

    opportunity_id: UUIDString
    amount: IntString
    executor: Address
    permission_key: HexString
    signature: SignedMessageString
    deadline: IntString
    nonce: IntString

    model_config = {
        "arbitrary_types_allowed": True,
    }


class OpportunityParamsV1(BaseModel):
    """
    Attributes:
        target_calldata: The calldata for the contract call.
        chain_id: The chain ID to bid on.
        target_contract: The contract address to call.
        permission_key: The permission key to bid on.
        buy_tokens: The tokens to receive in the opportunity.
        sell_tokens: The tokens to spend in the opportunity.
        target_call_value: The value to send with the contract call.
        version: The version of the opportunity.
    """

    target_calldata: HexString
    chain_id: str
    target_contract: Address
    permission_key: HexString
    buy_tokens: list[TokenAmount]
    sell_tokens: list[TokenAmount]
    target_call_value: IntString
    version: Literal["v1"]


class OpportunityParams(BaseModel):
    """
    Attributes:
        params: The parameters of the opportunity.
    """

    params: Union[OpportunityParamsV1] = Field(..., discriminator="version")


Opportunity = Union[OpportunityEvm, OpportunitySvm]
OpportunityRoot = RootModel[Opportunity]
OpportunityDelete = Union[OpportunityDeleteEvm | OpportunityDeleteSvm]
OpportunityDeleteRoot = RootModel[OpportunityDelete]


class SubscribeMessageParams(BaseModel):
    """
    Attributes:
        method: A string literal "subscribe".
        chain_ids: The chain IDs to subscribe to.
    """

    method: Literal["subscribe"]
    chain_ids: list[str]


class UnsubscribeMessageParams(BaseModel):
    """
    Attributes:
        method: A string literal "unsubscribe".
        chain_ids: The chain IDs to subscribe to.
    """

    method: Literal["unsubscribe"]
    chain_ids: list[str]


class PostBidMessageParamsEvm(BaseModel):
    """
    Attributes:
        method: A string literal "post_bid".
        amount: The amount of the bid in wei.
        target_calldata: The calldata for the contract call.
        chain_id: The chain ID to bid on.
        target_contract: The contract address to call.
        permission_key: The permission key to bid on.
    """

    method: Literal["post_bid"]
    amount: IntString
    target_calldata: HexString
    chain_id: str
    target_contract: Address
    permission_key: HexString


class PostOnChainBidMessageParamsSvm(BaseModel):
    """
    Attributes:
        method: A string literal "post_bid".
        chain_id: The chain ID to bid on.
        transaction: The transaction including the bid.
        slot: The minimum slot required for the bid to be executed successfully
              None if the bid can be executed at any recent slot
    """

    method: Literal["post_bid"]
    chain_id: str
    transaction: SvmTransaction
    slot: int | None


class PostSwapBidMessageParamsSvm(BaseModel):
    """
    Attributes:
        method: A string literal "post_bid".
        chain_id: The chain ID to bid on.
        transaction: The transaction including the bid.
        opportunity_id: The ID of the swap opportunity.
    """

    method: Literal["post_bid"]
    type: Literal["swap"]
    chain_id: str
    transaction: SvmTransaction
    opportunity_id: UUIDString


def get_discriminator_value(v: Any) -> str:
    if isinstance(v, dict):
        if "transaction" in v:
            return "svm"
        return "evm"
    if getattr(v, "transaction", None):
        return "svm"
    return "evm"


PostBidMessageParams = Annotated[
    Union[
        Annotated[PostBidMessageParamsEvm, Tag("evm")],
        Annotated[
            Union[PostOnChainBidMessageParamsSvm, PostSwapBidMessageParamsSvm],
            Tag("svm"),
        ],
    ],
    Discriminator(get_discriminator_value),
]


class PostOpportunityBidMessageParams(BaseModel):
    """
    Attributes:
        method: A string literal "post_opportunity_bid".
        opportunity_id: The ID of the opportunity.
        amount: The amount of the bid in wei.
        executor: The address of the executor.
        permission_key: The permission key to bid on.
        signature: The signature of the bid.
        deadline: The unix timestamp after which the bid becomes invalid.
        nonce: The nonce of the bid.
    """

    method: Literal["post_opportunity_bid"]
    opportunity_id: UUIDString
    amount: IntString
    executor: Address
    permission_key: HexString
    signature: SignedMessageString
    deadline: IntString
    nonce: IntString

    model_config = {
        "arbitrary_types_allowed": True,
    }


class ClientMessage(BaseModel):
    """
    Attributes:
        params: The parameters of the message.
    """

    params: Union[
        SubscribeMessageParams,
        UnsubscribeMessageParams,
        PostBidMessageParams,
        PostOpportunityBidMessageParams,
    ] = Field(..., discriminator="method")


class OpportunityAdapterConfig(BaseModel):
    """
    Attributes:
        chain_id: The chain ID.
        opportunity_adapter_factory: The address of the opportunity adapter factory contract.
        opportunity_adapter_init_bytecode_hash: The hash of the init bytecode of the opportunity adapter.
        permit2: The address of the permit2 contract.
        weth: The address of the WETH contract.
    """

    chain_id: int
    opportunity_adapter_factory: Address
    opportunity_adapter_init_bytecode_hash: Bytes32
    permit2: Address
    weth: Address
