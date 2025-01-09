from __future__ import annotations
import typing
from solders.pubkey import Pubkey
from solders.instruction import Instruction, AccountMeta
import borsh_construct as borsh
from .. import types
from ..program_id import PROGRAM_ID


class SetSwapPlatformFeeArgs(typing.TypedDict):
    data: types.set_swap_platform_fee_args.SetSwapPlatformFeeArgs


layout = borsh.CStruct(
    "data" / types.set_swap_platform_fee_args.SetSwapPlatformFeeArgs.layout
)


class SetSwapPlatformFeeAccounts(typing.TypedDict):
    admin: Pubkey
    express_relay_metadata: Pubkey


def set_swap_platform_fee(
    args: SetSwapPlatformFeeArgs,
    accounts: SetSwapPlatformFeeAccounts,
    program_id: Pubkey = PROGRAM_ID,
    remaining_accounts: typing.Optional[typing.List[AccountMeta]] = None,
) -> Instruction:
    keys: list[AccountMeta] = [
        AccountMeta(pubkey=accounts["admin"], is_signer=True, is_writable=False),
        AccountMeta(
            pubkey=accounts["express_relay_metadata"], is_signer=False, is_writable=True
        ),
    ]
    if remaining_accounts is not None:
        keys += remaining_accounts
    identifier = b"\x02\x87K\x0f\x08i\x8e/"
    encoded_args = layout.build(
        {
            "data": args["data"].to_encodable(),
        }
    )
    data = identifier + encoded_args
    return Instruction(program_id, data, keys)
