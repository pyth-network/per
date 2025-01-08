from __future__ import annotations
import typing
from dataclasses import dataclass
from anchorpy.borsh_extension import EnumForCodegen
import borsh_construct as borsh


class InputJSON(typing.TypedDict):
    kind: typing.Literal["Input"]


class OutputJSON(typing.TypedDict):
    kind: typing.Literal["Output"]


@dataclass
class Input:
    discriminator: typing.ClassVar = 0
    kind: typing.ClassVar = "Input"

    @classmethod
    def to_json(cls) -> InputJSON:
        return InputJSON(
            kind="Input",
        )

    @classmethod
    def to_encodable(cls) -> dict:
        return {
            "Input": {},
        }


@dataclass
class Output:
    discriminator: typing.ClassVar = 1
    kind: typing.ClassVar = "Output"

    @classmethod
    def to_json(cls) -> OutputJSON:
        return OutputJSON(
            kind="Output",
        )

    @classmethod
    def to_encodable(cls) -> dict:
        return {
            "Output": {},
        }


FeeTokenKind = typing.Union[Input, Output]
FeeTokenJSON = typing.Union[InputJSON, OutputJSON]


def from_decoded(obj: dict) -> FeeTokenKind:
    if not isinstance(obj, dict):
        raise ValueError("Invalid enum object")
    if "Input" in obj:
        return Input()
    if "Output" in obj:
        return Output()
    raise ValueError("Invalid enum object")


def from_json(obj: FeeTokenJSON) -> FeeTokenKind:
    if obj["kind"] == "Input":
        return Input()
    if obj["kind"] == "Output":
        return Output()
    kind = obj["kind"]
    raise ValueError(f"Unrecognized enum kind: {kind}")


layout = EnumForCodegen("Input" / borsh.CStruct(), "Output" / borsh.CStruct())
