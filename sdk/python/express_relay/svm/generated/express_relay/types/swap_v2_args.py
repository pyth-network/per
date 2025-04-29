from __future__ import annotations
from . import (
    fee_token,
)
import typing
from dataclasses import dataclass
from construct import Container
import borsh_construct as borsh


class SwapV2ArgsJSON(typing.TypedDict):
    deadline: int
    amount_searcher: int
    amount_user: int
    referral_fee_ppm: int
    fee_token: fee_token.FeeTokenJSON
    swap_platform_fee_ppm: int


@dataclass
class SwapV2Args:
    layout: typing.ClassVar = borsh.CStruct(
        "deadline" / borsh.I64,
        "amount_searcher" / borsh.U64,
        "amount_user" / borsh.U64,
        "referral_fee_ppm" / borsh.U64,
        "fee_token" / fee_token.layout,
        "swap_platform_fee_ppm" / borsh.U64,
    )
    deadline: int
    amount_searcher: int
    amount_user: int
    referral_fee_ppm: int
    fee_token: fee_token.FeeTokenKind
    swap_platform_fee_ppm: int

    @classmethod
    def from_decoded(cls, obj: Container) -> "SwapV2Args":
        return cls(
            deadline=obj.deadline,
            amount_searcher=obj.amount_searcher,
            amount_user=obj.amount_user,
            referral_fee_ppm=obj.referral_fee_ppm,
            fee_token=fee_token.from_decoded(obj.fee_token),
            swap_platform_fee_ppm=obj.swap_platform_fee_ppm,
        )

    def to_encodable(self) -> dict[str, typing.Any]:
        return {
            "deadline": self.deadline,
            "amount_searcher": self.amount_searcher,
            "amount_user": self.amount_user,
            "referral_fee_ppm": self.referral_fee_ppm,
            "fee_token": self.fee_token.to_encodable(),
            "swap_platform_fee_ppm": self.swap_platform_fee_ppm,
        }

    def to_json(self) -> SwapV2ArgsJSON:
        return {
            "deadline": self.deadline,
            "amount_searcher": self.amount_searcher,
            "amount_user": self.amount_user,
            "referral_fee_ppm": self.referral_fee_ppm,
            "fee_token": self.fee_token.to_json(),
            "swap_platform_fee_ppm": self.swap_platform_fee_ppm,
        }

    @classmethod
    def from_json(cls, obj: SwapV2ArgsJSON) -> "SwapV2Args":
        return cls(
            deadline=obj["deadline"],
            amount_searcher=obj["amount_searcher"],
            amount_user=obj["amount_user"],
            referral_fee_ppm=obj["referral_fee_ppm"],
            fee_token=fee_token.from_json(obj["fee_token"]),
            swap_platform_fee_ppm=obj["swap_platform_fee_ppm"],
        )
