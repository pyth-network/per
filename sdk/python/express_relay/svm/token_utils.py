from solana.constants import SYSTEM_PROGRAM_ID
from solders.instruction import AccountMeta, Instruction
from solders.pubkey import Pubkey
from solders.sysvar import RENT
from spl.token.constants import ASSOCIATED_TOKEN_PROGRAM_ID


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
