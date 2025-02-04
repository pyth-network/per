from __future__ import annotations
import typing
from dataclasses import dataclass
from anchorpy.borsh_extension import EnumForCodegen
import borsh_construct as borsh


class SearcherJSON(typing.TypedDict):
    kind: typing.Literal["Searcher"]


class UserJSON(typing.TypedDict):
    kind: typing.Literal["User"]


@dataclass
class Searcher:
    discriminator: typing.ClassVar = 0
    kind: typing.ClassVar = "Searcher"

    @classmethod
    def to_json(cls) -> SearcherJSON:
        return SearcherJSON(
            kind="Searcher",
        )

    @classmethod
    def to_encodable(cls) -> dict:
        return {
            "Searcher": {},
        }


@dataclass
class User:
    discriminator: typing.ClassVar = 1
    kind: typing.ClassVar = "User"

    @classmethod
    def to_json(cls) -> UserJSON:
        return UserJSON(
            kind="User",
        )

    @classmethod
    def to_encodable(cls) -> dict:
        return {
            "User": {},
        }


FeeTokenKind = typing.Union[Searcher, User]
FeeTokenJSON = typing.Union[SearcherJSON, UserJSON]


def from_decoded(obj: dict) -> FeeTokenKind:
    if not isinstance(obj, dict):
        raise ValueError("Invalid enum object")
    if "Searcher" in obj:
        return Searcher()
    if "User" in obj:
        return User()
    raise ValueError("Invalid enum object")


def from_json(obj: FeeTokenJSON) -> FeeTokenKind:
    if obj["kind"] == "Searcher":
        return Searcher()
    if obj["kind"] == "User":
        return User()
    kind = obj["kind"]
    raise ValueError(f"Unrecognized enum kind: {kind}")


layout = EnumForCodegen("Searcher" / borsh.CStruct(), "User" / borsh.CStruct())
