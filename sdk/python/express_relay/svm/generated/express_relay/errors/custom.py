import typing
from anchorpy.error import ProgramError


class FeeSplitLargerThanPrecision(ProgramError):
    def __init__(self) -> None:
        super().__init__(6000, "Fee split(s) larger than fee precision")

    code = 6000
    name = "FeeSplitLargerThanPrecision"
    msg = "Fee split(s) larger than fee precision"


class FeesHigherThanBid(ProgramError):
    def __init__(self) -> None:
        super().__init__(6001, "Fees higher than bid")

    code = 6001
    name = "FeesHigherThanBid"
    msg = "Fees higher than bid"


class DeadlinePassed(ProgramError):
    def __init__(self) -> None:
        super().__init__(6002, "Deadline passed")

    code = 6002
    name = "DeadlinePassed"
    msg = "Deadline passed"


class InvalidCPISubmitBid(ProgramError):
    def __init__(self) -> None:
        super().__init__(6003, "Invalid CPI into submit bid instruction")

    code = 6003
    name = "InvalidCPISubmitBid"
    msg = "Invalid CPI into submit bid instruction"


class MissingPermission(ProgramError):
    def __init__(self) -> None:
        super().__init__(6004, "Missing permission")

    code = 6004
    name = "MissingPermission"
    msg = "Missing permission"


class MultiplePermissions(ProgramError):
    def __init__(self) -> None:
        super().__init__(6005, "Multiple permissions")

    code = 6005
    name = "MultiplePermissions"
    msg = "Multiple permissions"


class InsufficientSearcherFunds(ProgramError):
    def __init__(self) -> None:
        super().__init__(6006, "Insufficient searcher funds")

    code = 6006
    name = "InsufficientSearcherFunds"
    msg = "Insufficient searcher funds"


class InsufficientRent(ProgramError):
    def __init__(self) -> None:
        super().__init__(6007, "Insufficient funds for rent")

    code = 6007
    name = "InsufficientRent"
    msg = "Insufficient funds for rent"


class InvalidAta(ProgramError):
    def __init__(self) -> None:
        super().__init__(6008, "Invalid ATA provided")

    code = 6008
    name = "InvalidAta"
    msg = "Invalid ATA provided"


class InvalidMint(ProgramError):
    def __init__(self) -> None:
        super().__init__(6009, "A token account has the wrong mint")

    code = 6009
    name = "InvalidMint"
    msg = "A token account has the wrong mint"


class InvalidTokenProgram(ProgramError):
    def __init__(self) -> None:
        super().__init__(6010, "A token account belongs to the wrong token program")

    code = 6010
    name = "InvalidTokenProgram"
    msg = "A token account belongs to the wrong token program"


CustomError = typing.Union[
    FeeSplitLargerThanPrecision,
    FeesHigherThanBid,
    DeadlinePassed,
    InvalidCPISubmitBid,
    MissingPermission,
    MultiplePermissions,
    InsufficientSearcherFunds,
    InsufficientRent,
    InvalidAta,
    InvalidMint,
    InvalidTokenProgram,
]
CUSTOM_ERROR_MAP: dict[int, CustomError] = {
    6000: FeeSplitLargerThanPrecision(),
    6001: FeesHigherThanBid(),
    6002: DeadlinePassed(),
    6003: InvalidCPISubmitBid(),
    6004: MissingPermission(),
    6005: MultiplePermissions(),
    6006: InsufficientSearcherFunds(),
    6007: InsufficientRent(),
    6008: InvalidAta(),
    6009: InvalidMint(),
    6010: InvalidTokenProgram(),
}


def from_code(code: int) -> typing.Optional[CustomError]:
    maybe_err = CUSTOM_ERROR_MAP.get(code)
    if maybe_err is None:
        return None
    return maybe_err
