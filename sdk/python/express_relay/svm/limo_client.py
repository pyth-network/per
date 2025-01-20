from typing import List, Sequence, Tuple, TypedDict

import spl.token.instructions as spl_token
from express_relay.svm.generated.limo.accounts import Order
from express_relay.svm.generated.limo.instructions import TakeOrderArgs, take_order
from express_relay.svm.generated.limo.program_id import PROGRAM_ID
from express_relay.svm.token_utils import (
    create_associated_token_account_idempotent,
    get_ata,
)
from solana.rpc.async_api import AsyncClient
from solana.rpc.types import MemcmpOpts
from solders import system_program
from solders.instruction import Instruction
from solders.pubkey import Pubkey
from solders.system_program import TransferParams
from solders.sysvar import INSTRUCTIONS
from spl.token._layouts import MINT_LAYOUT
from spl.token.constants import TOKEN_PROGRAM_ID, WRAPPED_SOL_MINT

ESCROW_VAULT_SEED = b"escrow_vault"
GLOBAL_AUTH_SEED = b"authority"
EXPRESS_RELAY_MEATADATA_SEED = b"metadata"
EXPRESS_RELAY_CONFIG_ROUTER_SEED = b"config_router"
EVENT_AUTH_SEED = b"__event_authority"
INTERMEDIARY_OUTPUT_TOKEN_ACCOUNT_SEED = b"intermediary"


class OrderStateAndAddress(TypedDict):
    state: Order
    address: Pubkey


class WSOLInstructions(TypedDict):
    create_ixs: List[Instruction]
    fill_ixs: List[Instruction]
    close_ixs: List[Instruction]
    ata: Pubkey


class LimoClient:
    def __init__(self, connection: AsyncClient):
        self._connection = connection

    async def get_all_orders_state_and_address_with_filters(
        self, filters: List[MemcmpOpts], global_config: Pubkey
    ) -> List[OrderStateAndAddress]:
        filters.append(MemcmpOpts(offset=8, bytes=str(global_config)))
        programs = await self._connection.get_program_accounts(
            PROGRAM_ID,
            commitment=None,
            encoding="base64",
            data_slice=None,
            filters=filters,
        )

        return [
            {"state": Order.decode(value.account.data), "address": value.pubkey}
            for value in programs.value
        ]

    async def get_mint_decimals(self, mint: Pubkey) -> int:
        mint_account = await self._connection.get_account_info(mint)
        if mint_account.value is None:
            raise ValueError("Mint account not found")
        bytes_data = mint_account.value.data
        if len(bytes_data) != MINT_LAYOUT.sizeof():
            raise ValueError("Invalid mint size")

        decoded_data = MINT_LAYOUT.parse(bytes_data)
        decimals = decoded_data.decimals
        return decimals

    def get_ata_and_create_ixn_if_required(
        self,
        owner: Pubkey,
        token_mint_address: Pubkey,
        token_program_id: Pubkey,
        payer: Pubkey,
    ) -> Tuple[Pubkey, Sequence[Instruction]]:
        ata = get_ata(owner, token_mint_address, token_program_id)
        ix = create_associated_token_account_idempotent(
            payer, owner, token_mint_address, token_program_id
        )
        return ata, [ix]

    def get_init_if_needed_wsol_create_and_close_ixs(
        self, owner: Pubkey, payer: Pubkey, amount_to_deposit_lamports: int
    ) -> WSOLInstructions:
        """
        Returns necessary instructions to create, fill and close a wrapped SOL account.
        Creation instruction is idempotent.
        Filling instruction doesn't take into account the current WSOL balance.
        Closing instruction always closes the WSOL account and unwraps all WSOL back to SOL.
        Args:
            owner: Who owns the WSOL token account
            payer: Who pays for the instructions
            amount_to_deposit_lamports: Amount of lamports to deposit into the WSOL account
        """
        ata = get_ata(owner, WRAPPED_SOL_MINT, TOKEN_PROGRAM_ID)

        create_ixs = [
            create_associated_token_account_idempotent(
                payer, owner, WRAPPED_SOL_MINT, TOKEN_PROGRAM_ID
            )
        ]

        fill_ixs = []
        if amount_to_deposit_lamports > 0 and payer == owner:
            fill_ixs = [
                system_program.transfer(
                    TransferParams(
                        from_pubkey=owner,
                        to_pubkey=ata,
                        lamports=amount_to_deposit_lamports,
                    )
                ),
                spl_token.sync_native(
                    spl_token.SyncNativeParams(TOKEN_PROGRAM_ID, ata)
                ),
            ]

        close_ixs = []
        if payer == owner:
            close_ixs = [
                spl_token.close_account(
                    spl_token.CloseAccountParams(
                        program_id=TOKEN_PROGRAM_ID,
                        account=ata,
                        dest=owner,
                        owner=owner,
                    )
                )
            ]
        return WSOLInstructions(
            create_ixs=create_ixs, fill_ixs=fill_ixs, close_ixs=close_ixs, ata=ata
        )

    def take_order_ix(
        self,
        taker: Pubkey,
        order: OrderStateAndAddress,
        input_amount: int,
        output_amount: int,
        express_relay_program_id: Pubkey,
    ) -> List[Instruction]:
        """
        Returns the instructions to fulfill an order as a taker.
        Args:
            taker: The taker's public key
            order: The order to fulfill
            input_amount: The amount of input tokens to take.
            output_amount: The amount of output tokens to provide.
            express_relay_program_id: Express relay program id

        Returns:
            A list of instructions to include in the transaction to fulfill the order. The submit_bid instruction for
            express relay program is not included and should be added separately.

        """
        ixs: List[Instruction] = []
        close_wsol_ixns: List[Instruction] = []
        taker_input_ata: Pubkey
        if order["state"].input_mint == WRAPPED_SOL_MINT:
            instructions = self.get_init_if_needed_wsol_create_and_close_ixs(
                owner=taker, payer=taker, amount_to_deposit_lamports=0
            )
            ixs.extend(instructions["create_ixs"])
            close_wsol_ixns.extend(instructions["close_ixs"])
            taker_input_ata = instructions["ata"]
        else:
            (
                taker_input_ata,
                create_taker_input_ata_ixs,
            ) = self.get_ata_and_create_ixn_if_required(
                owner=taker,
                token_mint_address=order["state"].input_mint,
                token_program_id=order["state"].input_mint_program_id,
                payer=taker,
            )
            ixs.extend(create_taker_input_ata_ixs)

        taker_output_ata: Pubkey
        maker_output_ata: Pubkey | None = None
        intermediary_output_token_account: Pubkey | None = None
        if order["state"].output_mint == WRAPPED_SOL_MINT:
            instructions = self.get_init_if_needed_wsol_create_and_close_ixs(
                owner=taker, payer=taker, amount_to_deposit_lamports=output_amount
            )
            ixs.extend(instructions["create_ixs"])
            ixs.extend(instructions["fill_ixs"])
            close_wsol_ixns.extend(instructions["close_ixs"])
            taker_output_ata = instructions["ata"]

            intermediary_output_token_account = self.get_intermediary_token_account_pda(
                PROGRAM_ID, order["address"]
            )
        else:
            (
                taker_output_ata,
                create_taker_output_ata_ixs,
            ) = self.get_ata_and_create_ixn_if_required(
                owner=taker,
                token_mint_address=order["state"].output_mint,
                token_program_id=order["state"].output_mint_program_id,
                payer=taker,
            )
            ixs.extend(create_taker_output_ata_ixs)

            (
                maker_output_ata,
                create_maker_output_ata_ixs,
            ) = self.get_ata_and_create_ixn_if_required(
                owner=order["state"].maker,
                token_mint_address=order["state"].output_mint,
                token_program_id=order["state"].output_mint_program_id,
                payer=taker,
            )
            ixs.extend(create_maker_output_ata_ixs)

        pda_authority = self.get_pda_authority(PROGRAM_ID, order["state"].global_config)
        ixs.append(
            take_order(
                TakeOrderArgs(
                    input_amount=input_amount,
                    min_output_amount=output_amount,
                    tip_amount_permissionless_taking=0,
                ),
                {
                    "taker": taker,
                    "maker": order["state"].maker,
                    "global_config": order["state"].global_config,
                    "pda_authority": pda_authority,
                    "order": order["address"],
                    "input_mint": order["state"].input_mint,
                    "output_mint": order["state"].output_mint,
                    "input_vault": self.get_token_vault_pda(
                        PROGRAM_ID,
                        order["state"].global_config,
                        order["state"].input_mint,
                    ),
                    "taker_input_ata": taker_input_ata,
                    "taker_output_ata": taker_output_ata,
                    "intermediary_output_token_account": intermediary_output_token_account,
                    "maker_output_ata": maker_output_ata,
                    "express_relay": express_relay_program_id,
                    "express_relay_metadata": self.get_express_relay_metadata_pda(
                        express_relay_program_id
                    ),
                    "sysvar_instructions": INSTRUCTIONS,
                    "permission": order["address"],
                    "config_router": self.get_express_relay_config_router_pda(
                        express_relay_program_id, pda_authority
                    ),
                    "input_token_program": order["state"].input_mint_program_id,
                    "output_token_program": order["state"].output_mint_program_id,
                    "event_authority": self.get_event_authority(PROGRAM_ID),
                    "program": PROGRAM_ID,
                },
            )
        )

        ixs.extend(close_wsol_ixns)
        return ixs

    @staticmethod
    def get_program_id() -> Pubkey:
        return PROGRAM_ID

    @staticmethod
    def get_token_vault_pda(
        program_id: Pubkey, global_config: Pubkey, input_mint: Pubkey
    ) -> Pubkey:
        return Pubkey.find_program_address(
            seeds=[ESCROW_VAULT_SEED, bytes(global_config), bytes(input_mint)],
            program_id=program_id,
        )[0]

    @staticmethod
    def get_express_relay_metadata_pda(program_id: Pubkey) -> Pubkey:
        return Pubkey.find_program_address(
            seeds=[EXPRESS_RELAY_MEATADATA_SEED], program_id=program_id
        )[0]

    @staticmethod
    def get_express_relay_config_router_pda(
        program_id: Pubkey, router: Pubkey
    ) -> Pubkey:
        return Pubkey.find_program_address(
            seeds=[EXPRESS_RELAY_CONFIG_ROUTER_SEED, bytes(router)],
            program_id=program_id,
        )[0]

    @staticmethod
    def get_pda_authority(program_id: Pubkey, global_config: Pubkey) -> Pubkey:
        return Pubkey.find_program_address(
            seeds=[GLOBAL_AUTH_SEED, bytes(global_config)], program_id=program_id
        )[0]

    @staticmethod
    def get_event_authority(program_id: Pubkey) -> Pubkey:
        return Pubkey.find_program_address(
            seeds=[EVENT_AUTH_SEED], program_id=program_id
        )[0]

    @staticmethod
    def get_intermediary_token_account_pda(
        program_id: Pubkey, order_address: Pubkey
    ) -> Pubkey:
        return Pubkey.find_program_address(
            seeds=[INTERMEDIARY_OUTPUT_TOKEN_ACCOUNT_SEED, bytes(order_address)],
            program_id=program_id,
        )[0]
