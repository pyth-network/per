from __future__ import annotations
import typing
from dataclasses import dataclass
from construct import Container
import borsh_construct as borsh


class SetSwapPlatformFeeArgsJSON(typing.TypedDict):
    swap_platform_fee_bps: int


@dataclass
class SetSwapPlatformFeeArgs:
    layout: typing.ClassVar = borsh.CStruct("swap_platform_fee_bps" / borsh.U64)
    swap_platform_fee_bps: int

    @classmethod
    def from_decoded(cls, obj: Container) -> "SetSwapPlatformFeeArgs":
        return cls(swap_platform_fee_bps=obj.swap_platform_fee_bps)

    def to_encodable(self) -> dict[str, typing.Any]:
        return {"swap_platform_fee_bps": self.swap_platform_fee_bps}

    def to_json(self) -> SetSwapPlatformFeeArgsJSON:
        return {"swap_platform_fee_bps": self.swap_platform_fee_bps}

    @classmethod
    def from_json(cls, obj: SetSwapPlatformFeeArgsJSON) -> "SetSwapPlatformFeeArgs":
        return cls(swap_platform_fee_bps=obj["swap_platform_fee_bps"])
