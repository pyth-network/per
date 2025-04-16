from typing import Any, Union

from express_relay.models.base import UUIDString
from express_relay.models.svm import (
    BidResponseSvm,
    BidStatusSvm,
    BidSvm,
    OpportunityDeleteSvm,
    OpportunitySvm,
    SvmTransaction,
    TokenAmountSvm,
)
from pydantic import BaseModel, Discriminator, Field, RootModel, Tag
from typing_extensions import Annotated, Literal

Bid = BidSvm


class BidCancel(BaseModel):
    """
    Attributes:
        bid_id: The ID of the bid to cancel.
        chain_id: The chain ID to cancel the bid on.
    """

    bid_id: UUIDString
    chain_id: str


class BidStatusUpdate(BaseModel):
    """
    Attributes:
        id: The ID of the bid.
        bid_status: The current status of the bid.
    """

    id: UUIDString
    bid_status: BidStatusSvm


BidResponse = BidResponseSvm
BidResponseRoot = RootModel[BidResponse]


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

    chain_id: str
    buy_tokens: list[TokenAmountSvm]
    sell_tokens: list[TokenAmountSvm]
    version: Literal["v1"]


class OpportunityParams(BaseModel):
    """
    Attributes:
        params: The parameters of the opportunity.
    """

    params: Union[OpportunityParamsV1] = Field(..., discriminator="version")


Opportunity = OpportunitySvm
OpportunityRoot = RootModel[Opportunity]
OpportunityDelete = OpportunityDeleteSvm
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
    if "opportunity_id" in v:
        return "swap"
    return "on_chain"


PostBidMessageParams = Annotated[
    Union[
        Annotated[PostSwapBidMessageParamsSvm, Tag("swap")],
        Annotated[
            PostOnChainBidMessageParamsSvm,
            Tag("on_chain"),
        ],
    ],
    Discriminator(get_discriminator_value),
]


class CancelBidMessageParams(BaseModel):
    """
    Attributes:
        method: A string literal "cancel_bid".
        data: The Cancel Bid data.
    """

    method: Literal["cancel_bid"]
    data: BidCancel


class ClientMessage(BaseModel):
    """
    Attributes:
        params: The parameters of the message.
    """

    params: Union[
        SubscribeMessageParams,
        UnsubscribeMessageParams,
        PostBidMessageParams,
        CancelBidMessageParams,
    ] = Field(..., discriminator="method")
