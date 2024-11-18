from __future__ import annotations
import typing
from solders.pubkey import Pubkey
from solders.instruction import Instruction, AccountMeta
from ..program_id import PROGRAM_ID


class UpdateGlobalConfigAdminAccounts(typing.TypedDict):
    admin_authority_cached: Pubkey
    global_config: Pubkey


def update_global_config_admin(
    accounts: UpdateGlobalConfigAdminAccounts,
    program_id: Pubkey = PROGRAM_ID,
    remaining_accounts: typing.Optional[typing.List[AccountMeta]] = None,
) -> Instruction:
    keys: list[AccountMeta] = [
        AccountMeta(
            pubkey=accounts["admin_authority_cached"], is_signer=True, is_writable=False
        ),
        AccountMeta(
            pubkey=accounts["global_config"], is_signer=False, is_writable=True
        ),
    ]
    if remaining_accounts is not None:
        keys += remaining_accounts
    identifier = b"\xb8W\x17\xc1\x9c\xee\xafw"
    encoded_args = b""
    data = identifier + encoded_args
    return Instruction(program_id, data, keys)
