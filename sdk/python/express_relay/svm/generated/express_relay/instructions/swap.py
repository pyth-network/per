from __future__ import annotations
import typing
from solders.pubkey import Pubkey
from solders.instruction import Instruction, AccountMeta
import borsh_construct as borsh
from .. import types
from ..program_id import PROGRAM_ID


class SwapArgs(typing.TypedDict):
    data: types.swap_args.SwapArgs


layout = borsh.CStruct("data" / types.swap_args.SwapArgs.layout)


class SwapAccounts(typing.TypedDict):
    searcher: Pubkey
    trader: Pubkey
    searcher_input_ta: Pubkey
    searcher_output_ta: Pubkey
    trader_input_ata: Pubkey
    trader_output_ata: Pubkey
    router_fee_receiver_ta: Pubkey
    relayer_fee_receiver_ata: Pubkey
    express_relay_fee_receiver_ata: Pubkey
    mint_input: Pubkey
    mint_output: Pubkey
    mint_fee: Pubkey
    token_program_input: Pubkey
    token_program_output: Pubkey
    token_program_fee: Pubkey
    express_relay_metadata: Pubkey


def swap(
    args: SwapArgs,
    accounts: SwapAccounts,
    program_id: Pubkey = PROGRAM_ID,
    remaining_accounts: typing.Optional[typing.List[AccountMeta]] = None,
) -> Instruction:
    keys: list[AccountMeta] = [
        AccountMeta(pubkey=accounts["searcher"], is_signer=True, is_writable=False),
        AccountMeta(pubkey=accounts["trader"], is_signer=True, is_writable=False),
        AccountMeta(
            pubkey=accounts["searcher_input_ta"], is_signer=False, is_writable=True
        ),
        AccountMeta(
            pubkey=accounts["searcher_output_ta"], is_signer=False, is_writable=True
        ),
        AccountMeta(
            pubkey=accounts["trader_input_ata"], is_signer=False, is_writable=True
        ),
        AccountMeta(
            pubkey=accounts["trader_output_ata"], is_signer=False, is_writable=True
        ),
        AccountMeta(
            pubkey=accounts["router_fee_receiver_ta"], is_signer=False, is_writable=True
        ),
        AccountMeta(
            pubkey=accounts["relayer_fee_receiver_ata"],
            is_signer=False,
            is_writable=True,
        ),
        AccountMeta(
            pubkey=accounts["express_relay_fee_receiver_ata"],
            is_signer=False,
            is_writable=True,
        ),
        AccountMeta(pubkey=accounts["mint_input"], is_signer=False, is_writable=False),
        AccountMeta(pubkey=accounts["mint_output"], is_signer=False, is_writable=False),
        AccountMeta(pubkey=accounts["mint_fee"], is_signer=False, is_writable=False),
        AccountMeta(
            pubkey=accounts["token_program_input"], is_signer=False, is_writable=False
        ),
        AccountMeta(
            pubkey=accounts["token_program_output"], is_signer=False, is_writable=False
        ),
        AccountMeta(
            pubkey=accounts["token_program_fee"], is_signer=False, is_writable=False
        ),
        AccountMeta(
            pubkey=accounts["express_relay_metadata"],
            is_signer=False,
            is_writable=False,
        ),
    ]
    if remaining_accounts is not None:
        keys += remaining_accounts
    identifier = b"\xf8\xc6\x9e\x91\xe1u\x87\xc8"
    encoded_args = layout.build(
        {
            "data": args["data"].to_encodable(),
        }
    )
    data = identifier + encoded_args
    return Instruction(program_id, data, keys)
