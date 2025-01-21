from enum import Enum
from typing import Annotated
from uuid import UUID

from pydantic import PlainSerializer


class UnsupportedOpportunityVersionException(Exception):
    pass


class UnsupportedOpportunityDeleteVersionException(Exception):
    pass


class UnsupportedOpportunityDeleteChainTypeException(Exception):
    pass


class BidStatusVariantsEvm(Enum):
    PENDING = "pending"
    SUBMITTED = "submitted"
    LOST = "lost"
    WON = "won"


class BidStatusVariantsSvm(Enum):
    PENDING = "pending"
    SUBMITTED = "submitted"
    LOST = "lost"
    WON = "won"
    FAILED = "failed"
    EXPIRED = "expired"


IntString = Annotated[int, PlainSerializer(lambda x: str(x), return_type=str)]
UUIDString = Annotated[UUID, PlainSerializer(lambda x: str(x), return_type=str)]
