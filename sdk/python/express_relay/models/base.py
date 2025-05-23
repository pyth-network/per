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


class BidStatusVariantsSvm(Enum):
    PENDING = "pending"
    AWAITING_SIGNATURE = "awaiting_signature"
    SENT_TO_USER_FOR_SUBMISSION = "sent_to_user_for_submission"
    SUBMITTED = "submitted"
    LOST = "lost"
    WON = "won"
    FAILED = "failed"
    EXPIRED = "expired"
    CANCELLED = "cancelled"
    SUBMISSION_FAILED = "submission_failed"
    UNKNOWN = "unknown"


class BidSubmissionFailedReasonVariantsSvm(Enum):
    CANCELLED = "cancelled"
    DEADLINE_PASSED = "deadline_passed"


class BidFailedReasonVariantsSvm(Enum):
    INSUFFICIENT_USER_FUNDS = "insufficient_user_funds"
    INSUFFICIENT_SEARCHER_FUNDS = "insufficient_searcher_funds"
    INSUFFICIENT_FUNDS_SOL_TRANSFER = "insufficient_funds_sol_transfer"
    DEADLINE_PASSED = "deadline_passed"
    OTHER = "other"


IntString = Annotated[int, PlainSerializer(lambda x: str(x), return_type=str)]
UUIDString = Annotated[UUID, PlainSerializer(lambda x: str(x), return_type=str)]
