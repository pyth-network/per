from __future__ import annotations
import typing
from solders.pubkey import Pubkey
from solders.instruction import Instruction, AccountMeta
import borsh_construct as borsh
from .. import types
from ..program_id import PROGRAM_ID


class SwapV2Args(typing.TypedDict):
    data: types.swap_v2_args.SwapV2Args


layout = borsh.CStruct("data" / types.swap_v2_args.SwapV2Args.layout)


class SwapV2Accounts(typing.TypedDict):
    searcher: Pubkey
    user: Pubkey
    searcher_ta_mint_searcher: Pubkey
    searcher_ta_mint_user: Pubkey
    user_ata_mint_searcher: Pubkey
    user_ata_mint_user: Pubkey
    router_fee_receiver_ta: Pubkey
    relayer_fee_receiver_ata: Pubkey
    express_relay_fee_receiver_ata: Pubkey
    mint_searcher: Pubkey
    mint_user: Pubkey
    mint_fee: Pubkey
    token_program_searcher: Pubkey
    token_program_user: Pubkey
    token_program_fee: Pubkey
    express_relay_metadata: Pubkey
    relayer_signer: Pubkey


def swap_v2(
    args: SwapV2Args,
    accounts: SwapV2Accounts,
    program_id: Pubkey = PROGRAM_ID,
    remaining_accounts: typing.Optional[typing.List[AccountMeta]] = None,
) -> Instruction:
    keys: list[AccountMeta] = [
        AccountMeta(pubkey=accounts["searcher"], is_signer=True, is_writable=False),
        AccountMeta(pubkey=accounts["user"], is_signer=True, is_writable=False),
        AccountMeta(
            pubkey=accounts["searcher_ta_mint_searcher"],
            is_signer=False,
            is_writable=True,
        ),
        AccountMeta(
            pubkey=accounts["searcher_ta_mint_user"], is_signer=False, is_writable=True
        ),
        AccountMeta(
            pubkey=accounts["user_ata_mint_searcher"], is_signer=False, is_writable=True
        ),
        AccountMeta(
            pubkey=accounts["user_ata_mint_user"], is_signer=False, is_writable=True
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
        AccountMeta(
            pubkey=accounts["mint_searcher"], is_signer=False, is_writable=False
        ),
        AccountMeta(pubkey=accounts["mint_user"], is_signer=False, is_writable=False),
        AccountMeta(pubkey=accounts["mint_fee"], is_signer=False, is_writable=False),
        AccountMeta(
            pubkey=accounts["token_program_searcher"],
            is_signer=False,
            is_writable=False,
        ),
        AccountMeta(
            pubkey=accounts["token_program_user"], is_signer=False, is_writable=False
        ),
        AccountMeta(
            pubkey=accounts["token_program_fee"], is_signer=False, is_writable=False
        ),
        AccountMeta(
            pubkey=accounts["express_relay_metadata"],
            is_signer=False,
            is_writable=False,
        ),
        AccountMeta(
            pubkey=accounts["relayer_signer"], is_signer=True, is_writable=False
        ),
    ]
    if remaining_accounts is not None:
        keys += remaining_accounts
    identifier = b"+\x04\xed\x0b\x1a\xc9\x1eb"
    encoded_args = layout.build(
        {
            "data": args["data"].to_encodable(),
        }
    )
    data = identifier + encoded_args
    return Instruction(program_id, data, keys)
