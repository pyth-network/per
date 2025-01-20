from __future__ import annotations
from . import (
    fee_token,
)
import typing
from dataclasses import dataclass
from construct import Container
import borsh_construct as borsh


class SwapArgsJSON(typing.TypedDict):
    deadline: int
    amount_input: int
    amount_output: int
    referral_fee_bps: int
    fee_token: fee_token.FeeTokenJSON


@dataclass
class SwapArgs:
    layout: typing.ClassVar = borsh.CStruct(
        "deadline" / borsh.I64,
        "amount_input" / borsh.U64,
        "amount_output" / borsh.U64,
        "referral_fee_bps" / borsh.U16,
        "fee_token" / fee_token.layout,
    )
    deadline: int
    amount_input: int
    amount_output: int
    referral_fee_bps: int
    fee_token: fee_token.FeeTokenKind

    @classmethod
    def from_decoded(cls, obj: Container) -> "SwapArgs":
        return cls(
            deadline=obj.deadline,
            amount_input=obj.amount_input,
            amount_output=obj.amount_output,
            referral_fee_bps=obj.referral_fee_bps,
            fee_token=fee_token.from_decoded(obj.fee_token),
        )

    def to_encodable(self) -> dict[str, typing.Any]:
        return {
            "deadline": self.deadline,
            "amount_input": self.amount_input,
            "amount_output": self.amount_output,
            "referral_fee_bps": self.referral_fee_bps,
            "fee_token": self.fee_token.to_encodable(),
        }

    def to_json(self) -> SwapArgsJSON:
        return {
            "deadline": self.deadline,
            "amount_input": self.amount_input,
            "amount_output": self.amount_output,
            "referral_fee_bps": self.referral_fee_bps,
            "fee_token": self.fee_token.to_json(),
        }

    @classmethod
    def from_json(cls, obj: SwapArgsJSON) -> "SwapArgs":
        return cls(
            deadline=obj["deadline"],
            amount_input=obj["amount_input"],
            amount_output=obj["amount_output"],
            referral_fee_bps=obj["referral_fee_bps"],
            fee_token=fee_token.from_json(obj["fee_token"]),
        )
