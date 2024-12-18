from __future__ import annotations
import typing
from solders.pubkey import Pubkey
from solders.system_program import ID as SYS_PROGRAM_ID
from solders.sysvar import RENT
from solders.instruction import Instruction, AccountMeta
import borsh_construct as borsh
from ..program_id import PROGRAM_ID


class TakeOrderArgs(typing.TypedDict):
    input_amount: int
    min_output_amount: int
    tip_amount_permissionless_taking: int


layout = borsh.CStruct(
    "input_amount" / borsh.U64,
    "min_output_amount" / borsh.U64,
    "tip_amount_permissionless_taking" / borsh.U64,
)


class TakeOrderAccounts(typing.TypedDict):
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
    intermediary_output_token_account: typing.Optional[Pubkey]
    maker_output_ata: typing.Optional[Pubkey]
    express_relay: Pubkey
    express_relay_metadata: Pubkey
    sysvar_instructions: Pubkey
    permission: typing.Optional[Pubkey]
    config_router: Pubkey
    input_token_program: Pubkey
    output_token_program: Pubkey
    event_authority: Pubkey
    program: Pubkey


def take_order(
    args: TakeOrderArgs,
    accounts: TakeOrderAccounts,
    program_id: Pubkey = PROGRAM_ID,
    remaining_accounts: typing.Optional[typing.List[AccountMeta]] = None,
) -> Instruction:
    keys: list[AccountMeta] = [
        AccountMeta(pubkey=accounts["taker"], is_signer=True, is_writable=True),
        AccountMeta(pubkey=accounts["maker"], is_signer=False, is_writable=True),
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
        (
            AccountMeta(
                pubkey=accounts["intermediary_output_token_account"],
                is_signer=False,
                is_writable=True,
            )
            if accounts["intermediary_output_token_account"]
            else AccountMeta(pubkey=program_id, is_signer=False, is_writable=False)
        ),
        (
            AccountMeta(
                pubkey=accounts["maker_output_ata"], is_signer=False, is_writable=True
            )
            if accounts["maker_output_ata"]
            else AccountMeta(pubkey=program_id, is_signer=False, is_writable=False)
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
        (
            AccountMeta(
                pubkey=accounts["permission"], is_signer=False, is_writable=False
            )
            if accounts["permission"]
            else AccountMeta(pubkey=program_id, is_signer=False, is_writable=False)
        ),
        AccountMeta(
            pubkey=accounts["config_router"], is_signer=False, is_writable=False
        ),
        AccountMeta(
            pubkey=accounts["input_token_program"], is_signer=False, is_writable=False
        ),
        AccountMeta(
            pubkey=accounts["output_token_program"], is_signer=False, is_writable=False
        ),
        AccountMeta(pubkey=RENT, is_signer=False, is_writable=False),
        AccountMeta(pubkey=SYS_PROGRAM_ID, is_signer=False, is_writable=False),
        AccountMeta(
            pubkey=accounts["event_authority"], is_signer=False, is_writable=False
        ),
        AccountMeta(pubkey=accounts["program"], is_signer=False, is_writable=False),
    ]
    if remaining_accounts is not None:
        keys += remaining_accounts
    identifier = b"\xa3\xd0\x14\xac\xdfA\xff\xe4"
    encoded_args = layout.build(
        {
            "input_amount": args["input_amount"],
            "min_output_amount": args["min_output_amount"],
            "tip_amount_permissionless_taking": args[
                "tip_amount_permissionless_taking"
            ],
        }
    )
    data = identifier + encoded_args
    return Instruction(program_id, data, keys)
