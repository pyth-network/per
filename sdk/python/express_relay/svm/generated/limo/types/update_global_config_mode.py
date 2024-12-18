from __future__ import annotations
import typing
from dataclasses import dataclass
from anchorpy.borsh_extension import EnumForCodegen
import borsh_construct as borsh


class UpdateEmergencyModeJSON(typing.TypedDict):
    kind: typing.Literal["UpdateEmergencyMode"]


class UpdateFlashTakeOrderBlockedJSON(typing.TypedDict):
    kind: typing.Literal["UpdateFlashTakeOrderBlocked"]


class UpdateBlockNewOrdersJSON(typing.TypedDict):
    kind: typing.Literal["UpdateBlockNewOrders"]


class UpdateBlockOrderTakingJSON(typing.TypedDict):
    kind: typing.Literal["UpdateBlockOrderTaking"]


class UpdateHostFeeBpsJSON(typing.TypedDict):
    kind: typing.Literal["UpdateHostFeeBps"]


class UpdateAdminAuthorityCachedJSON(typing.TypedDict):
    kind: typing.Literal["UpdateAdminAuthorityCached"]


class UpdateOrderTakingPermissionlessJSON(typing.TypedDict):
    kind: typing.Literal["UpdateOrderTakingPermissionless"]


class UpdateOrderCloseDelaySecondsJSON(typing.TypedDict):
    kind: typing.Literal["UpdateOrderCloseDelaySeconds"]


@dataclass
class UpdateEmergencyMode:
    discriminator: typing.ClassVar = 0
    kind: typing.ClassVar = "UpdateEmergencyMode"

    @classmethod
    def to_json(cls) -> UpdateEmergencyModeJSON:
        return UpdateEmergencyModeJSON(
            kind="UpdateEmergencyMode",
        )

    @classmethod
    def to_encodable(cls) -> dict:
        return {
            "UpdateEmergencyMode": {},
        }


@dataclass
class UpdateFlashTakeOrderBlocked:
    discriminator: typing.ClassVar = 1
    kind: typing.ClassVar = "UpdateFlashTakeOrderBlocked"

    @classmethod
    def to_json(cls) -> UpdateFlashTakeOrderBlockedJSON:
        return UpdateFlashTakeOrderBlockedJSON(
            kind="UpdateFlashTakeOrderBlocked",
        )

    @classmethod
    def to_encodable(cls) -> dict:
        return {
            "UpdateFlashTakeOrderBlocked": {},
        }


@dataclass
class UpdateBlockNewOrders:
    discriminator: typing.ClassVar = 2
    kind: typing.ClassVar = "UpdateBlockNewOrders"

    @classmethod
    def to_json(cls) -> UpdateBlockNewOrdersJSON:
        return UpdateBlockNewOrdersJSON(
            kind="UpdateBlockNewOrders",
        )

    @classmethod
    def to_encodable(cls) -> dict:
        return {
            "UpdateBlockNewOrders": {},
        }


@dataclass
class UpdateBlockOrderTaking:
    discriminator: typing.ClassVar = 3
    kind: typing.ClassVar = "UpdateBlockOrderTaking"

    @classmethod
    def to_json(cls) -> UpdateBlockOrderTakingJSON:
        return UpdateBlockOrderTakingJSON(
            kind="UpdateBlockOrderTaking",
        )

    @classmethod
    def to_encodable(cls) -> dict:
        return {
            "UpdateBlockOrderTaking": {},
        }


@dataclass
class UpdateHostFeeBps:
    discriminator: typing.ClassVar = 4
    kind: typing.ClassVar = "UpdateHostFeeBps"

    @classmethod
    def to_json(cls) -> UpdateHostFeeBpsJSON:
        return UpdateHostFeeBpsJSON(
            kind="UpdateHostFeeBps",
        )

    @classmethod
    def to_encodable(cls) -> dict:
        return {
            "UpdateHostFeeBps": {},
        }


@dataclass
class UpdateAdminAuthorityCached:
    discriminator: typing.ClassVar = 5
    kind: typing.ClassVar = "UpdateAdminAuthorityCached"

    @classmethod
    def to_json(cls) -> UpdateAdminAuthorityCachedJSON:
        return UpdateAdminAuthorityCachedJSON(
            kind="UpdateAdminAuthorityCached",
        )

    @classmethod
    def to_encodable(cls) -> dict:
        return {
            "UpdateAdminAuthorityCached": {},
        }


@dataclass
class UpdateOrderTakingPermissionless:
    discriminator: typing.ClassVar = 6
    kind: typing.ClassVar = "UpdateOrderTakingPermissionless"

    @classmethod
    def to_json(cls) -> UpdateOrderTakingPermissionlessJSON:
        return UpdateOrderTakingPermissionlessJSON(
            kind="UpdateOrderTakingPermissionless",
        )

    @classmethod
    def to_encodable(cls) -> dict:
        return {
            "UpdateOrderTakingPermissionless": {},
        }


@dataclass
class UpdateOrderCloseDelaySeconds:
    discriminator: typing.ClassVar = 7
    kind: typing.ClassVar = "UpdateOrderCloseDelaySeconds"

    @classmethod
    def to_json(cls) -> UpdateOrderCloseDelaySecondsJSON:
        return UpdateOrderCloseDelaySecondsJSON(
            kind="UpdateOrderCloseDelaySeconds",
        )

    @classmethod
    def to_encodable(cls) -> dict:
        return {
            "UpdateOrderCloseDelaySeconds": {},
        }


UpdateGlobalConfigModeKind = typing.Union[
    UpdateEmergencyMode,
    UpdateFlashTakeOrderBlocked,
    UpdateBlockNewOrders,
    UpdateBlockOrderTaking,
    UpdateHostFeeBps,
    UpdateAdminAuthorityCached,
    UpdateOrderTakingPermissionless,
    UpdateOrderCloseDelaySeconds,
]
UpdateGlobalConfigModeJSON = typing.Union[
    UpdateEmergencyModeJSON,
    UpdateFlashTakeOrderBlockedJSON,
    UpdateBlockNewOrdersJSON,
    UpdateBlockOrderTakingJSON,
    UpdateHostFeeBpsJSON,
    UpdateAdminAuthorityCachedJSON,
    UpdateOrderTakingPermissionlessJSON,
    UpdateOrderCloseDelaySecondsJSON,
]


def from_decoded(obj: dict) -> UpdateGlobalConfigModeKind:
    if not isinstance(obj, dict):
        raise ValueError("Invalid enum object")
    if "UpdateEmergencyMode" in obj:
        return UpdateEmergencyMode()
    if "UpdateFlashTakeOrderBlocked" in obj:
        return UpdateFlashTakeOrderBlocked()
    if "UpdateBlockNewOrders" in obj:
        return UpdateBlockNewOrders()
    if "UpdateBlockOrderTaking" in obj:
        return UpdateBlockOrderTaking()
    if "UpdateHostFeeBps" in obj:
        return UpdateHostFeeBps()
    if "UpdateAdminAuthorityCached" in obj:
        return UpdateAdminAuthorityCached()
    if "UpdateOrderTakingPermissionless" in obj:
        return UpdateOrderTakingPermissionless()
    if "UpdateOrderCloseDelaySeconds" in obj:
        return UpdateOrderCloseDelaySeconds()
    raise ValueError("Invalid enum object")


def from_json(obj: UpdateGlobalConfigModeJSON) -> UpdateGlobalConfigModeKind:
    if obj["kind"] == "UpdateEmergencyMode":
        return UpdateEmergencyMode()
    if obj["kind"] == "UpdateFlashTakeOrderBlocked":
        return UpdateFlashTakeOrderBlocked()
    if obj["kind"] == "UpdateBlockNewOrders":
        return UpdateBlockNewOrders()
    if obj["kind"] == "UpdateBlockOrderTaking":
        return UpdateBlockOrderTaking()
    if obj["kind"] == "UpdateHostFeeBps":
        return UpdateHostFeeBps()
    if obj["kind"] == "UpdateAdminAuthorityCached":
        return UpdateAdminAuthorityCached()
    if obj["kind"] == "UpdateOrderTakingPermissionless":
        return UpdateOrderTakingPermissionless()
    if obj["kind"] == "UpdateOrderCloseDelaySeconds":
        return UpdateOrderCloseDelaySeconds()
    kind = obj["kind"]
    raise ValueError(f"Unrecognized enum kind: {kind}")


layout = EnumForCodegen(
    "UpdateEmergencyMode" / borsh.CStruct(),
    "UpdateFlashTakeOrderBlocked" / borsh.CStruct(),
    "UpdateBlockNewOrders" / borsh.CStruct(),
    "UpdateBlockOrderTaking" / borsh.CStruct(),
    "UpdateHostFeeBps" / borsh.CStruct(),
    "UpdateAdminAuthorityCached" / borsh.CStruct(),
    "UpdateOrderTakingPermissionless" / borsh.CStruct(),
    "UpdateOrderCloseDelaySeconds" / borsh.CStruct(),
)
