import typing
from anchorpy.error import ProgramError


class OrderCanNotBeCanceled(ProgramError):
    def __init__(self) -> None:
        super().__init__(6000, "Order can't be canceled")

    code = 6000
    name = "OrderCanNotBeCanceled"
    msg = "Order can't be canceled"


class OrderNotActive(ProgramError):
    def __init__(self) -> None:
        super().__init__(6001, "Order not active")

    code = 6001
    name = "OrderNotActive"
    msg = "Order not active"


class InvalidAdminAuthority(ProgramError):
    def __init__(self) -> None:
        super().__init__(6002, "Invalid admin authority")

    code = 6002
    name = "InvalidAdminAuthority"
    msg = "Invalid admin authority"


class InvalidPdaAuthority(ProgramError):
    def __init__(self) -> None:
        super().__init__(6003, "Invalid pda authority")

    code = 6003
    name = "InvalidPdaAuthority"
    msg = "Invalid pda authority"


class InvalidConfigOption(ProgramError):
    def __init__(self) -> None:
        super().__init__(6004, "Invalid config option")

    code = 6004
    name = "InvalidConfigOption"
    msg = "Invalid config option"


class InvalidOrderOwner(ProgramError):
    def __init__(self) -> None:
        super().__init__(6005, "Order owner account is not the order owner")

    code = 6005
    name = "InvalidOrderOwner"
    msg = "Order owner account is not the order owner"


class OutOfRangeIntegralConversion(ProgramError):
    def __init__(self) -> None:
        super().__init__(6006, "Out of range integral conversion attempted")

    code = 6006
    name = "OutOfRangeIntegralConversion"
    msg = "Out of range integral conversion attempted"


class InvalidFlag(ProgramError):
    def __init__(self) -> None:
        super().__init__(6007, "Invalid boolean flag, valid values are 0 and 1")

    code = 6007
    name = "InvalidFlag"
    msg = "Invalid boolean flag, valid values are 0 and 1"


class MathOverflow(ProgramError):
    def __init__(self) -> None:
        super().__init__(6008, "Mathematical operation with overflow")

    code = 6008
    name = "MathOverflow"
    msg = "Mathematical operation with overflow"


class OrderInputAmountInvalid(ProgramError):
    def __init__(self) -> None:
        super().__init__(6009, "Order input amount invalid")

    code = 6009
    name = "OrderInputAmountInvalid"
    msg = "Order input amount invalid"


class OrderOutputAmountInvalid(ProgramError):
    def __init__(self) -> None:
        super().__init__(6010, "Order output amount invalid")

    code = 6010
    name = "OrderOutputAmountInvalid"
    msg = "Order output amount invalid"


class InvalidHostFee(ProgramError):
    def __init__(self) -> None:
        super().__init__(6011, "Host fee bps must be between 0 and 10000")

    code = 6011
    name = "InvalidHostFee"
    msg = "Host fee bps must be between 0 and 10000"


class IntegerOverflow(ProgramError):
    def __init__(self) -> None:
        super().__init__(6012, "Conversion between integers failed")

    code = 6012
    name = "IntegerOverflow"
    msg = "Conversion between integers failed"


class InvalidTipBalance(ProgramError):
    def __init__(self) -> None:
        super().__init__(6013, "Tip balance less than accounted tip")

    code = 6013
    name = "InvalidTipBalance"
    msg = "Tip balance less than accounted tip"


class InvalidTipTransferAmount(ProgramError):
    def __init__(self) -> None:
        super().__init__(6014, "Tip transfer amount is less than expected")

    code = 6014
    name = "InvalidTipTransferAmount"
    msg = "Tip transfer amount is less than expected"


class InvalidHostTipBalance(ProgramError):
    def __init__(self) -> None:
        super().__init__(6015, "Host tup amount is less than accounted for")

    code = 6015
    name = "InvalidHostTipBalance"
    msg = "Host tup amount is less than accounted for"


class OrderWithinFlashOperation(ProgramError):
    def __init__(self) -> None:
        super().__init__(
            6016, "Order within flash operation - all otehr actions are blocked"
        )

    code = 6016
    name = "OrderWithinFlashOperation"
    msg = "Order within flash operation - all otehr actions are blocked"


class CPINotAllowed(ProgramError):
    def __init__(self) -> None:
        super().__init__(6017, "CPI not allowed")

    code = 6017
    name = "CPINotAllowed"
    msg = "CPI not allowed"


class FlashTakeOrderBlocked(ProgramError):
    def __init__(self) -> None:
        super().__init__(6018, "Flash take_order is blocked")

    code = 6018
    name = "FlashTakeOrderBlocked"
    msg = "Flash take_order is blocked"


class FlashTxWithUnexpectedIxs(ProgramError):
    def __init__(self) -> None:
        super().__init__(
            6019,
            "Some unexpected instructions are present in the tx. Either before or after the flash ixs, or some ix target the same program between",
        )

    code = 6019
    name = "FlashTxWithUnexpectedIxs"
    msg = "Some unexpected instructions are present in the tx. Either before or after the flash ixs, or some ix target the same program between"


class FlashIxsNotEnded(ProgramError):
    def __init__(self) -> None:
        super().__init__(
            6020, "Flash ixs initiated without the closing ix in the transaction"
        )

    code = 6020
    name = "FlashIxsNotEnded"
    msg = "Flash ixs initiated without the closing ix in the transaction"


class FlashIxsNotStarted(ProgramError):
    def __init__(self) -> None:
        super().__init__(
            6021, "Flash ixs ended without the starting ix in the transaction"
        )

    code = 6021
    name = "FlashIxsNotStarted"
    msg = "Flash ixs ended without the starting ix in the transaction"


class FlashIxsAccountMismatch(ProgramError):
    def __init__(self) -> None:
        super().__init__(6022, "Some accounts differ between the two flash ixs")

    code = 6022
    name = "FlashIxsAccountMismatch"
    msg = "Some accounts differ between the two flash ixs"


class FlashIxsArgsMismatch(ProgramError):
    def __init__(self) -> None:
        super().__init__(6023, "Some args differ between the two flash ixs")

    code = 6023
    name = "FlashIxsArgsMismatch"
    msg = "Some args differ between the two flash ixs"


class OrderNotWithinFlashOperation(ProgramError):
    def __init__(self) -> None:
        super().__init__(6024, "Order is not within flash operation")

    code = 6024
    name = "OrderNotWithinFlashOperation"
    msg = "Order is not within flash operation"


class EmergencyModeEnabled(ProgramError):
    def __init__(self) -> None:
        super().__init__(6025, "Emergency mode is enabled")

    code = 6025
    name = "EmergencyModeEnabled"
    msg = "Emergency mode is enabled"


class CreatingNewOrdersBlocked(ProgramError):
    def __init__(self) -> None:
        super().__init__(6026, "Creating new ordersis blocked")

    code = 6026
    name = "CreatingNewOrdersBlocked"
    msg = "Creating new ordersis blocked"


class OrderTakingBlocked(ProgramError):
    def __init__(self) -> None:
        super().__init__(6027, "Orders taking is blocked")

    code = 6027
    name = "OrderTakingBlocked"
    msg = "Orders taking is blocked"


class OrderInputAmountTooLarge(ProgramError):
    def __init__(self) -> None:
        super().__init__(6028, "Order input amount larger than the remaining")

    code = 6028
    name = "OrderInputAmountTooLarge"
    msg = "Order input amount larger than the remaining"


class PermissionRequiredPermissionlessNotEnabled(ProgramError):
    def __init__(self) -> None:
        super().__init__(
            6029,
            "Permissionless order taking not enabled, please provide permission account",
        )

    code = 6029
    name = "PermissionRequiredPermissionlessNotEnabled"
    msg = "Permissionless order taking not enabled, please provide permission account"


class PermissionDoesNotMatchOrder(ProgramError):
    def __init__(self) -> None:
        super().__init__(6030, "Permission address does not match order address")

    code = 6030
    name = "PermissionDoesNotMatchOrder"
    msg = "Permission address does not match order address"


class InvalidAtaAddress(ProgramError):
    def __init__(self) -> None:
        super().__init__(6031, "Invalid ata address")

    code = 6031
    name = "InvalidAtaAddress"
    msg = "Invalid ata address"


class MakerOutputAtaRequired(ProgramError):
    def __init__(self) -> None:
        super().__init__(6032, "Maker output ata required when output mint is not WSOL")

    code = 6032
    name = "MakerOutputAtaRequired"
    msg = "Maker output ata required when output mint is not WSOL"


class IntermediaryOutputTokenAccountRequired(ProgramError):
    def __init__(self) -> None:
        super().__init__(
            6033, "Intermediary output token account required when output mint is WSOL"
        )

    code = 6033
    name = "IntermediaryOutputTokenAccountRequired"
    msg = "Intermediary output token account required when output mint is WSOL"


class NotEnoughBalanceForRent(ProgramError):
    def __init__(self) -> None:
        super().__init__(6034, "Not enough balance for rent")

    code = 6034
    name = "NotEnoughBalanceForRent"
    msg = "Not enough balance for rent"


class NotEnoughTimePassedSinceLastUpdate(ProgramError):
    def __init__(self) -> None:
        super().__init__(
            6035, "Order can not be closed - Not enough time passed since last update"
        )

    code = 6035
    name = "NotEnoughTimePassedSinceLastUpdate"
    msg = "Order can not be closed - Not enough time passed since last update"


CustomError = typing.Union[
    OrderCanNotBeCanceled,
    OrderNotActive,
    InvalidAdminAuthority,
    InvalidPdaAuthority,
    InvalidConfigOption,
    InvalidOrderOwner,
    OutOfRangeIntegralConversion,
    InvalidFlag,
    MathOverflow,
    OrderInputAmountInvalid,
    OrderOutputAmountInvalid,
    InvalidHostFee,
    IntegerOverflow,
    InvalidTipBalance,
    InvalidTipTransferAmount,
    InvalidHostTipBalance,
    OrderWithinFlashOperation,
    CPINotAllowed,
    FlashTakeOrderBlocked,
    FlashTxWithUnexpectedIxs,
    FlashIxsNotEnded,
    FlashIxsNotStarted,
    FlashIxsAccountMismatch,
    FlashIxsArgsMismatch,
    OrderNotWithinFlashOperation,
    EmergencyModeEnabled,
    CreatingNewOrdersBlocked,
    OrderTakingBlocked,
    OrderInputAmountTooLarge,
    PermissionRequiredPermissionlessNotEnabled,
    PermissionDoesNotMatchOrder,
    InvalidAtaAddress,
    MakerOutputAtaRequired,
    IntermediaryOutputTokenAccountRequired,
    NotEnoughBalanceForRent,
    NotEnoughTimePassedSinceLastUpdate,
]
CUSTOM_ERROR_MAP: dict[int, CustomError] = {
    6000: OrderCanNotBeCanceled(),
    6001: OrderNotActive(),
    6002: InvalidAdminAuthority(),
    6003: InvalidPdaAuthority(),
    6004: InvalidConfigOption(),
    6005: InvalidOrderOwner(),
    6006: OutOfRangeIntegralConversion(),
    6007: InvalidFlag(),
    6008: MathOverflow(),
    6009: OrderInputAmountInvalid(),
    6010: OrderOutputAmountInvalid(),
    6011: InvalidHostFee(),
    6012: IntegerOverflow(),
    6013: InvalidTipBalance(),
    6014: InvalidTipTransferAmount(),
    6015: InvalidHostTipBalance(),
    6016: OrderWithinFlashOperation(),
    6017: CPINotAllowed(),
    6018: FlashTakeOrderBlocked(),
    6019: FlashTxWithUnexpectedIxs(),
    6020: FlashIxsNotEnded(),
    6021: FlashIxsNotStarted(),
    6022: FlashIxsAccountMismatch(),
    6023: FlashIxsArgsMismatch(),
    6024: OrderNotWithinFlashOperation(),
    6025: EmergencyModeEnabled(),
    6026: CreatingNewOrdersBlocked(),
    6027: OrderTakingBlocked(),
    6028: OrderInputAmountTooLarge(),
    6029: PermissionRequiredPermissionlessNotEnabled(),
    6030: PermissionDoesNotMatchOrder(),
    6031: InvalidAtaAddress(),
    6032: MakerOutputAtaRequired(),
    6033: IntermediaryOutputTokenAccountRequired(),
    6034: NotEnoughBalanceForRent(),
    6035: NotEnoughTimePassedSinceLastUpdate(),
}


def from_code(code: int) -> typing.Optional[CustomError]:
    maybe_err = CUSTOM_ERROR_MAP.get(code)
    if maybe_err is None:
        return None
    return maybe_err
