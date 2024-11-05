from __future__ import annotations
import typing
from solders.pubkey import Pubkey
from solders.instruction import Instruction, AccountMeta
import borsh_construct as borsh
from ..program_id import PROGRAM_ID


class FlashTakeOrderEndArgs(typing.TypedDict):
    input_amount: int
    min_output_amount: int


layout = borsh.CStruct("input_amount" / borsh.U64, "min_output_amount" / borsh.U64)


class FlashTakeOrderEndAccounts(typing.TypedDict):
    taker: Pubkey
    maker: Pubkey
    global_config: Pubkey
    pda_authority: Pubkey
    order: Pubkey
    input_mint: Pubkey
    output_mint: Pubkey
    input_vault: Pubkey
    taker_input_ata: Pubkey
    taker_output_ata: Pubkey
    maker_output_ata: Pubkey
    express_relay: Pubkey
    express_relay_metadata: Pubkey
    sysvar_instructions: Pubkey
    permission: Pubkey
    config_router: Pubkey
    input_token_program: Pubkey
    output_token_program: Pubkey


def flash_take_order_end(
    args: FlashTakeOrderEndArgs,
    accounts: FlashTakeOrderEndAccounts,
    program_id: Pubkey = PROGRAM_ID,
    remaining_accounts: typing.Optional[typing.List[AccountMeta]] = None,
) -> Instruction:
    keys: list[AccountMeta] = [
        AccountMeta(pubkey=accounts["taker"], is_signer=True, is_writable=True),
        AccountMeta(pubkey=accounts["maker"], is_signer=False, is_writable=False),
        AccountMeta(
            pubkey=accounts["global_config"], is_signer=False, is_writable=True
        ),
        AccountMeta(
            pubkey=accounts["pda_authority"], is_signer=False, is_writable=True
        ),
        AccountMeta(pubkey=accounts["order"], is_signer=False, is_writable=True),
        AccountMeta(pubkey=accounts["input_mint"], is_signer=False, is_writable=False),
        AccountMeta(pubkey=accounts["output_mint"], is_signer=False, is_writable=False),
        AccountMeta(pubkey=accounts["input_vault"], is_signer=False, is_writable=True),
        AccountMeta(
            pubkey=accounts["taker_input_ata"], is_signer=False, is_writable=True
        ),
        AccountMeta(
            pubkey=accounts["taker_output_ata"], is_signer=False, is_writable=True
        ),
        AccountMeta(
            pubkey=accounts["maker_output_ata"], is_signer=False, is_writable=True
        ),
        AccountMeta(
            pubkey=accounts["express_relay"], is_signer=False, is_writable=False
        ),
        AccountMeta(
            pubkey=accounts["express_relay_metadata"],
            is_signer=False,
            is_writable=False,
        ),
        AccountMeta(
            pubkey=accounts["sysvar_instructions"], is_signer=False, is_writable=False
        ),
        AccountMeta(pubkey=accounts["permission"], is_signer=False, is_writable=False),
        AccountMeta(
            pubkey=accounts["config_router"], is_signer=False, is_writable=False
        ),
        AccountMeta(
            pubkey=accounts["input_token_program"], is_signer=False, is_writable=False
        ),
        AccountMeta(
            pubkey=accounts["output_token_program"], is_signer=False, is_writable=False
        ),
    ]
    if remaining_accounts is not None:
        keys += remaining_accounts
    identifier = b"\xce\xf2\xd7\xbb\x86!\xe0\x94"
    encoded_args = layout.build(
        {
            "input_amount": args["input_amount"],
            "min_output_amount": args["min_output_amount"],
        }
    )
    data = identifier + encoded_args
    return Instruction(program_id, data, keys)
