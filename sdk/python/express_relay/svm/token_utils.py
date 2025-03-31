from typing import List

from solana.constants import SYSTEM_PROGRAM_ID
from solders.instruction import AccountMeta, Instruction
from solders.pubkey import Pubkey
from solders.system_program import TransferParams, transfer
from solders.sysvar import RENT
from spl.token.constants import (
    ASSOCIATED_TOKEN_PROGRAM_ID,
    TOKEN_PROGRAM_ID,
    WRAPPED_SOL_MINT,
)
from spl.token.instructions import (
    CloseAccountParams,
    SyncNativeParams,
    close_account,
    sync_native,
)

RENT_TOKEN_ACCOUNT_LAMPORTS = 2039280


def get_ata(
    owner: Pubkey, token_mint_address: Pubkey, token_program_id: Pubkey
) -> Pubkey:
    ata, _ = Pubkey.find_program_address(
        seeds=[bytes(owner), bytes(token_program_id), bytes(token_mint_address)],
        program_id=ASSOCIATED_TOKEN_PROGRAM_ID,
    )
    return ata


def create_associated_token_account_idempotent(
    payer: Pubkey, owner: Pubkey, mint: Pubkey, token_program_id: Pubkey
) -> Instruction:
    """Creates a transaction instruction to create an associated token account.

    Returns:
        The instruction to create the associated token account.
    """
    associated_token_address = get_ata(owner, mint, token_program_id)
    return Instruction(
        accounts=[
            AccountMeta(pubkey=payer, is_signer=True, is_writable=True),
            AccountMeta(
                pubkey=associated_token_address, is_signer=False, is_writable=True
            ),
            AccountMeta(pubkey=owner, is_signer=False, is_writable=False),
            AccountMeta(pubkey=mint, is_signer=False, is_writable=False),
            AccountMeta(pubkey=SYSTEM_PROGRAM_ID, is_signer=False, is_writable=False),
            AccountMeta(pubkey=token_program_id, is_signer=False, is_writable=False),
            AccountMeta(pubkey=RENT, is_signer=False, is_writable=False),
        ],
        program_id=ASSOCIATED_TOKEN_PROGRAM_ID,
        data=bytes([1]),  # idempotent version of the instruction
    )


def wrap_sol(
    payer: Pubkey,
    owner: Pubkey,
    amount: int,
    create_ata: bool = True,
) -> List[Instruction]:
    """Creates transaction instructions to transfer and wrap SOL into an associated token account.

    Returns:
        The instructions to wrap SOL into an associated token account.
    """
    instructions = []
    if create_ata:
        instructions.append(
            create_associated_token_account_idempotent(
                payer, owner, WRAPPED_SOL_MINT, TOKEN_PROGRAM_ID
            )
        )

    ata = get_ata(owner, WRAPPED_SOL_MINT, TOKEN_PROGRAM_ID)
    instructions.append(
        transfer(
            TransferParams(
                from_pubkey=owner,
                to_pubkey=ata,
                lamports=amount,
            )
        )
    )
    instructions.append(
        sync_native(
            SyncNativeParams(
                program_id=TOKEN_PROGRAM_ID,
                account=ata,
            )
        )
    )
    return instructions


def unwrap_sol(owner: Pubkey) -> Instruction:
    """Creates a transaction instruction to close a wrapped SOL account.

    Returns:
        The instruction to close the wrapped SOL account.
    """
    ata = get_ata(owner, WRAPPED_SOL_MINT, TOKEN_PROGRAM_ID)
    return close_account(
        CloseAccountParams(
            program_id=TOKEN_PROGRAM_ID,
            account=ata,
            dest=owner,
            owner=owner,
        )
    )
