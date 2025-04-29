from __future__ import annotations
import typing
from solders.pubkey import Pubkey
from solders.instruction import Instruction, AccountMeta
from ..program_id import PROGRAM_ID


class SetSecondaryRelayerAccounts(typing.TypedDict):
    admin: Pubkey
    express_relay_metadata: Pubkey
    secondary_relayer_signer: Pubkey


def set_secondary_relayer(
    accounts: SetSecondaryRelayerAccounts,
    program_id: Pubkey = PROGRAM_ID,
    remaining_accounts: typing.Optional[typing.List[AccountMeta]] = None,
) -> Instruction:
    keys: list[AccountMeta] = [
        AccountMeta(pubkey=accounts["admin"], is_signer=True, is_writable=False),
        AccountMeta(
            pubkey=accounts["express_relay_metadata"], is_signer=False, is_writable=True
        ),
        AccountMeta(
            pubkey=accounts["secondary_relayer_signer"],
            is_signer=False,
            is_writable=False,
        ),
    ]
    if remaining_accounts is not None:
        keys += remaining_accounts
    identifier = b"\x94U\xebS\xcaN\xe3\xe8"
    encoded_args = b""
    data = identifier + encoded_args
    return Instruction(program_id, data, keys)
