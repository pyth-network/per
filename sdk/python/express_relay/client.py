import asyncio
import json
import urllib.parse
import warnings
from asyncio import Task
from collections.abc import Coroutine
from datetime import datetime
from typing import Any, Callable, List, TypedDict
from uuid import UUID

import express_relay.svm.generated.express_relay.types.fee_token as swap_fee_token
import httpx
import websockets
from express_relay.constants import SVM_CONFIGS
from express_relay.models import (
    Bid,
    BidResponse,
    BidResponseRoot,
    BidStatusUpdate,
    ClientMessage,
    Opportunity,
    OpportunityDelete,
    OpportunityDeleteRoot,
    OpportunityParams,
    OpportunityRoot,
)
from express_relay.models.base import UnsupportedOpportunityVersionException
from express_relay.models.svm import (
    SvmChainUpdate,
    SwapOpportunitySvm,
    TokenAccountInitializationConfig,
    TokenAccountInitializationConfigs,
)
from express_relay.svm.generated.express_relay.instructions.submit_bid import submit_bid
from express_relay.svm.generated.express_relay.instructions.swap import swap
from express_relay.svm.generated.express_relay.program_id import (
    PROGRAM_ID as SVM_EXPRESS_RELAY_PROGRAM_ID,
)
from express_relay.svm.generated.express_relay.types.submit_bid_args import (
    SubmitBidArgs,
)
from express_relay.svm.generated.express_relay.types.swap_args import SwapArgs
from express_relay.svm.limo_client import LimoClient
from express_relay.svm.token_utils import (
    RENT_TOKEN_ACCOUNT_LAMPORTS,
    create_associated_token_account_idempotent,
    get_ata,
    unwrap_sol,
    wrap_sol,
)
from solders.instruction import Instruction
from solders.pubkey import Pubkey
from solders.sysvar import INSTRUCTIONS
from spl.memo.constants import MEMO_PROGRAM_ID
from spl.token.constants import WRAPPED_SOL_MINT
from websockets.client import WebSocketClientProtocol


class ExpressRelayClientException(Exception):
    pass


class SwapAccounts(TypedDict):
    searcher_token: Pubkey
    token_program_searcher: Pubkey
    token_program_user: Pubkey
    user_token: Pubkey
    user: Pubkey
    mint_fee: Pubkey
    fee_token_program: Pubkey
    router: Pubkey


FEE_SPLIT_PRECISION = 10000


class TokenAccountToCreate(TypedDict):
    payer: Pubkey
    owner: Pubkey
    mint: Pubkey
    program: Pubkey


class TokenAccountInitializationParams(TypedDict):
    owner: Pubkey
    mint: Pubkey
    program: Pubkey
    config: TokenAccountInitializationConfig


class ExpressRelayClient:
    def __init__(
        self,
        server_url: str,
        api_key: str | None = None,
        opportunity_callback: (
            Callable[[Opportunity], Coroutine[Any, Any, Any]] | None
        ) = None,
        bid_status_callback: (
            Callable[[BidStatusUpdate], Coroutine[Any, Any, Any]] | None
        ) = None,
        svm_chain_update_callback: (
            Callable[[SvmChainUpdate], Coroutine[Any, Any, Any]] | None
        ) = None,
        remove_opportunities_callback: (
            Callable[[OpportunityDelete], Coroutine[Any, Any, Any]] | None
        ) = None,
        timeout_response_secs: int = 10,
        ws_options: dict[str, Any] | None = None,
        http_options: dict[str, Any] | None = None,
    ):
        """
        Args:
            server_url: The URL of the auction server.
            opportunity_callback: An async function that serves as the callback on a new opportunity. Should take in one external argument of type Opportunity.
            bid_status_callback: An async function that serves as the callback on a new bid status update. Should take in one external argument of type BidStatusUpdate.
            svm_chain_update_callback: An async function that serves as the callback on a new svm chain update. Should take in one external argument of type SvmChainUpdate.
            remove_opportunities_callback: An async function that serves as the callback on an opportunities delete. Should take in one external argument of type OpportunityDelete.
            timeout_response_secs: The number of seconds to wait for a response message from the server.
            ws_options: Keyword arguments to pass to the websocket connection.
            http_options: Keyword arguments to pass to the HTTP client.
        """
        parsed_url = urllib.parse.urlparse(server_url)
        if parsed_url.scheme == "https":
            ws_scheme = "wss"
        elif parsed_url.scheme == "http":
            ws_scheme = "ws"
        else:
            raise ValueError("Invalid server URL")

        self.server_url = server_url
        self.api_key = api_key
        self.ws_endpoint = parsed_url._replace(scheme=ws_scheme, path="/v1/ws").geturl()
        self.ws_msg_counter = 0
        self.ws: WebSocketClientProtocol
        self.ws_lock = asyncio.Lock()
        self.ws_loop: Task[Any]
        self.ws_msg_futures: dict[str, asyncio.Future] = {}
        self.timeout_response_secs = timeout_response_secs
        if ws_options is None:
            ws_options = {}
        self.ws_options = ws_options
        if http_options is None:
            http_options = {}
        self.http_options = http_options
        self.opportunity_callback = opportunity_callback
        self.bid_status_callback = bid_status_callback
        self.svm_chain_update_callback = svm_chain_update_callback
        self.remove_opportunities_callback = remove_opportunities_callback
        if self.api_key:
            authorization_header = f"Bearer {self.api_key}"
            if "headers" not in self.http_options:
                self.http_options["headers"] = {}
            self.http_options["headers"]["Authorization"] = authorization_header
            if "extra_headers" not in self.ws_options:
                self.ws_options["extra_headers"] = {}
            self.ws_options["extra_headers"]["Authorization"] = authorization_header

    async def start_ws(self):
        """
        Initializes the websocket connection to the server, if not already connected.
        """
        async with self.ws_lock:
            if not hasattr(self, "ws"):
                self.ws = await websockets.connect(self.ws_endpoint, **self.ws_options)

            if not hasattr(self, "ws_loop"):
                ws_call = self.ws_handler(
                    self.opportunity_callback,
                    self.bid_status_callback,
                    self.svm_chain_update_callback,
                    self.remove_opportunities_callback,
                )
                self.ws_loop = asyncio.create_task(ws_call)

    async def close_ws(self):
        """
        Closes the websocket connection to the server.
        """
        async with self.ws_lock:
            await self.ws.close()

    async def get_ws_loop(self) -> asyncio.Task:
        """
        Returns the websocket handler loop.
        """
        await self.start_ws()

        return self.ws_loop

    def convert_client_msg_to_server(self, client_msg: ClientMessage) -> dict:
        """
        Converts the params of a ClientMessage model dict to the format expected by the server.

        Args:
            client_msg: The message to send to the server.
        Returns:
            The message as a dict with the params converted to the format expected by the server.
        """
        msg = client_msg.model_dump()
        method = msg["params"]["method"]
        msg["id"] = str(self.ws_msg_counter)
        self.ws_msg_counter += 1

        if method == "post_bid":
            msg["params"] = {"bid": msg["params"]}

        msg["method"] = method

        return msg

    async def send_ws_msg(self, client_msg: ClientMessage) -> dict:
        """
        Sends a message to the server via websocket.

        Args:
            client_msg: The message to send.
        Returns:
            The result of the response message from the server.
        """
        await self.start_ws()

        msg = self.convert_client_msg_to_server(client_msg)

        future = asyncio.get_event_loop().create_future()
        self.ws_msg_futures[msg["id"]] = future

        await self.ws.send(json.dumps(msg))

        # await the response for the sent ws message from the server
        msg_response = await asyncio.wait_for(
            future, timeout=self.timeout_response_secs
        )

        return self.process_response_msg(msg_response)

    def process_response_msg(self, msg: dict) -> dict:
        """
        Processes a response message received from the server via websocket.

        Args:
            msg: The message to process.
        Returns:
            The result field of the message.
        """
        if msg.get("status") and msg.get("status") != "success":
            raise ExpressRelayClientException(
                f"Error in websocket response with message id {msg.get('id')}: {msg.get('result')}"
            )
        return msg["result"]

    async def subscribe_chains(self, chain_ids: list[str]):
        """
        Subscribes websocket to a list of chain IDs for new opportunities.

        Args:
            chain_ids: A list of chain IDs to subscribe to.
        """
        params = {
            "method": "subscribe",
            "chain_ids": chain_ids,
        }
        client_msg = ClientMessage.model_validate({"params": params})
        await self.send_ws_msg(client_msg)

    async def unsubscribe_chains(self, chain_ids: list[str]):
        """
        Unsubscribes websocket from a list of chain IDs for new opportunities.

        Args:
            chain_ids: A list of chain IDs to unsubscribe from.
        """
        params = {
            "method": "unsubscribe",
            "chain_ids": chain_ids,
        }
        client_msg = ClientMessage.model_validate({"params": params})
        await self.send_ws_msg(client_msg)

    async def submit_bid(self, bid: Bid, subscribe_to_updates: bool = True) -> UUID:
        """
        Submits a bid to the auction server.

        Args:
            bid: An object representing the bid to submit.
            subscribe_to_updates: A boolean indicating whether to subscribe to the bid status updates.
        Returns:
            The ID of the submitted bid.
        """
        bid_dict = bid.model_dump()
        if subscribe_to_updates:
            bid_dict["method"] = "post_bid"
            client_msg = ClientMessage.model_validate({"params": bid_dict})
            result = await self.send_ws_msg(client_msg)
            bid_id = UUID(result.get("id"))
        else:
            async with httpx.AsyncClient(**self.http_options) as client:
                resp = await client.post(
                    urllib.parse.urlparse(self.server_url)
                    ._replace(path="/v1/bids")
                    .geturl(),
                    json=bid_dict,
                )

            resp.raise_for_status()
            bid_id = UUID(resp.json().get("id"))

        return bid_id

    async def cancel_bid(self, bid_id: UUID, chain_id: str):
        """
        Cancels a bid on the auction server.

        Args:
            bid_id: The ID of the bid to cancel.
            chain_id: The chain ID of the bid to cancel.
        """
        params = {
            "method": "cancel_bid",
            "data": {
                "bid_id": bid_id,
                "chain_id": chain_id,
            },
        }
        client_msg = ClientMessage.model_validate({"params": params})
        await self.send_ws_msg(client_msg)

    async def ws_handler(
        self,
        opportunity_callback: (
            Callable[[Opportunity], Coroutine[Any, Any, Any]] | None
        ) = None,
        bid_status_callback: (
            Callable[[BidStatusUpdate], Coroutine[Any, Any, Any]] | None
        ) = None,
        svm_chain_update_callback: (
            Callable[[SvmChainUpdate], Coroutine[Any, Any, Any]] | None
        ) = None,
        remove_opportunities_callback: (
            Callable[[OpportunityDelete], Coroutine[Any, Any, Any]] | None
        ) = None,
    ):
        """
        Continually handles new ws messages as they are received from the server via websocket.

        Args:
            opportunity_callback: An async function that serves as the callback on a new opportunity. Should take in one external argument of type Opportunity.
            bid_status_callback: An async function that serves as the callback on a new bid status update. Should take in one external argument of type BidStatusUpdate.
            svm_chain_update_callback: An async function that serves as the callback on a new svm chain update. Should take in one external argument of type SvmChainUpdate.
            remove_opportunities_callback: An async function that serves as the callback on an opportunities delete. Should take in one external argument of type OpportunityDelete.
        """
        if not self.ws:
            raise ExpressRelayClientException("Websocket not connected")

        async for msg in self.ws:
            msg_json = json.loads(msg)

            if msg_json.get("type"):
                if msg_json.get("type") == "new_opportunity":
                    if opportunity_callback is not None:
                        opportunity = OpportunityRoot.model_validate(
                            msg_json["opportunity"]
                        )
                        if opportunity:
                            asyncio.create_task(opportunity_callback(opportunity.root))

                elif msg_json.get("type") == "bid_status_update":
                    if bid_status_callback is not None:
                        bid_status_update = BidStatusUpdate.model_validate(
                            msg_json["status"]
                        )
                        asyncio.create_task(bid_status_callback(bid_status_update))

                elif msg_json.get("type") == "svm_chain_update":
                    if svm_chain_update_callback is not None:
                        svm_chain_update = SvmChainUpdate.model_validate(
                            msg_json["update"]
                        )
                        asyncio.create_task(svm_chain_update_callback(svm_chain_update))

                elif msg_json.get("type") == "remove_opportunities":
                    if remove_opportunities_callback is not None:
                        remove_opportunities = OpportunityDeleteRoot.model_validate(
                            msg_json["opportunity_delete"]
                        )
                        if remove_opportunities:
                            asyncio.create_task(
                                remove_opportunities_callback(remove_opportunities.root)
                            )

            elif msg_json.get("id"):
                future = self.ws_msg_futures.pop(msg_json["id"])
                future.set_result(msg_json)

    async def get_opportunities(
        self,
        chain_id: str | None = None,
        from_time: datetime | None = None,
        limit: int | None = None,
    ) -> list[Opportunity]:
        """
        Connects to the server and fetches opportunities.

        Args:
            chain_id: The chain ID to fetch opportunities for. If None, fetches opportunities across all chains.
            from_time: The datetime to fetch opportunities from. If None, fetches from the beginning of time.
            limit: Number of opportunities to fetch. If None, uses the default server limit.
        Returns:
            A list of opportunities.
        """
        params = {}
        if chain_id:
            params["chain_id"] = chain_id
        if from_time:
            params["from_time"] = from_time.astimezone().isoformat(
                timespec="microseconds"
            )
        if limit:
            params["limit"] = str(limit)

        async with httpx.AsyncClient(**self.http_options) as client:
            resp = await client.get(
                urllib.parse.urlparse(self.server_url)
                ._replace(path="/v1/opportunities")
                .geturl(),
                params=params,
            )

        resp.raise_for_status()

        opportunities: list[Opportunity] = []
        for opportunity in resp.json():
            try:
                opportunity_processed = OpportunityRoot.model_validate(opportunity)
                opportunities.append(opportunity_processed.root)
            except UnsupportedOpportunityVersionException as e:
                warnings.warn(str(e))
        return opportunities

    async def submit_opportunity(self, opportunity: OpportunityParams) -> UUID:
        """
        Submits an opportunity to the server.

        Args:
            opportunity: An object representing the opportunity to submit.
        Returns:
            The ID of the submitted opportunity.
        """
        async with httpx.AsyncClient(**self.http_options) as client:
            resp = await client.post(
                urllib.parse.urlparse(self.server_url)
                ._replace(path="/v1/opportunities")
                .geturl(),
                json=opportunity.params.model_dump(),
            )
        resp.raise_for_status()
        return UUID(resp.json()["opportunity_id"])

    async def get_bids(self, from_time: datetime | None = None) -> list[BidResponse]:
        """
        Fetches bids for an api key from the server with pagination of 20 bids per page.

        Args:
            from_time: The datetime to fetch bids from. If None, fetches from the beginning of time.
        Returns:
            A list of bids.
        """
        async with httpx.AsyncClient(**self.http_options) as client:
            resp = await client.get(
                urllib.parse.urlparse(self.server_url)
                ._replace(path="/v1/bids")
                .geturl(),
                params=(
                    {"from_time": from_time.astimezone().isoformat()}
                    if from_time
                    else None
                ),
            )

        resp.raise_for_status()

        bids = []
        for bid in resp.json()["items"]:
            bid_processed = BidResponseRoot.model_validate(bid)
            bids.append(bid_processed.root)

        return bids

    @staticmethod
    def get_svm_submit_bid_instruction(
        searcher: Pubkey,
        router: Pubkey,
        permission_key: Pubkey,
        bid_amount: int,
        deadline: int,
        chain_id: str,
        fee_receiver_relayer: Pubkey,
        relayer_signer: Pubkey,
    ) -> Instruction:
        if chain_id not in SVM_CONFIGS:
            raise ValueError(f"Chain ID {chain_id} not supported")
        svm_config = SVM_CONFIGS[chain_id]
        config_router = LimoClient.get_express_relay_config_router_pda(
            SVM_EXPRESS_RELAY_PROGRAM_ID, router
        )
        express_relay_metadata = LimoClient.get_express_relay_metadata_pda(
            SVM_EXPRESS_RELAY_PROGRAM_ID
        )
        submit_bid_ix = submit_bid(
            {"data": SubmitBidArgs(deadline=deadline, bid_amount=bid_amount)},
            {
                "searcher": searcher,
                "relayer_signer": relayer_signer,
                "permission": permission_key,
                "router": router,
                "config_router": config_router,
                "express_relay_metadata": express_relay_metadata,
                "fee_receiver_relayer": fee_receiver_relayer,
                "sysvar_instructions": INSTRUCTIONS,
            },
            svm_config["express_relay_program"],
        )
        return submit_bid_ix

    @staticmethod
    def get_token_account_to_create(
        searcher: Pubkey,
        user: Pubkey,
        params: TokenAccountInitializationParams,
    ) -> TokenAccountToCreate | None:
        if params["config"] == "unneeded":
            return None
        return {
            "payer": searcher if params["config"] == "searcher_payer" else user,
            "owner": params["owner"],
            "mint": params["mint"],
            "program": params["program"],
        }

    @staticmethod
    def get_token_accounts_to_create(
        searcher: Pubkey,
        swap_opportunity: SwapOpportunitySvm,
        fee_receiver_relayer: Pubkey,
        express_relay_metadata: Pubkey,
        configs: TokenAccountInitializationConfigs,
    ) -> List[TokenAccountToCreate]:
        accs = ExpressRelayClient.extract_swap_info(swap_opportunity)
        token_accounts_initialization_params: List[TokenAccountInitializationParams] = [
            TokenAccountInitializationParams(
                owner=accs["user"],
                mint=accs["searcher_token"],
                program=accs["token_program_searcher"],
                config=configs.user_ata_mint_searcher,
            ),
            TokenAccountInitializationParams(
                owner=accs["user"],
                mint=accs["user_token"],
                program=accs["token_program_user"],
                config=configs.user_ata_mint_user,
            ),
            TokenAccountInitializationParams(
                owner=accs["router"],
                mint=accs["mint_fee"],
                program=accs["fee_token_program"],
                config=configs.router_fee_receiver_ta,
            ),
            TokenAccountInitializationParams(
                owner=fee_receiver_relayer,
                mint=accs["mint_fee"],
                program=accs["fee_token_program"],
                config=configs.relayer_fee_receiver_ata,
            ),
            TokenAccountInitializationParams(
                owner=express_relay_metadata,
                mint=accs["mint_fee"],
                program=accs["fee_token_program"],
                config=configs.express_relay_fee_receiver_ata,
            ),
        ]

        return list(
            filter(
                lambda x: x is not None,
                [
                    ExpressRelayClient.get_token_account_to_create(
                        searcher=searcher, user=accs["user"], params=params
                    )
                    for params in token_accounts_initialization_params
                ],
            )
        )

    @staticmethod
    def get_user_amount_to_wrap(
        amount_user: int,
        user_mint_user_balance: int,
        token_account_initialization_configs: TokenAccountInitializationConfigs,
    ) -> int:
        number_of_atas_paid_by_user = len(
            [
                x
                for x in [
                    token_account_initialization_configs.user_ata_mint_user,
                    token_account_initialization_configs.user_ata_mint_searcher,
                ]
                if x == "user_payer"
            ]
        )

        return min(
            amount_user,
            max(
                0,
                user_mint_user_balance
                - number_of_atas_paid_by_user * RENT_TOKEN_ACCOUNT_LAMPORTS,
            ),
        )

    @staticmethod
    def extract_swap_info(swap_opportunity: SwapOpportunitySvm) -> SwapAccounts:
        token_program_searcher = swap_opportunity.tokens.token_program_searcher
        token_program_user = swap_opportunity.tokens.token_program_user
        searcher_token = swap_opportunity.tokens.searcher_token
        user_token = swap_opportunity.tokens.user_token
        user = swap_opportunity.user_wallet_address
        mint_fee, fee_token_program = (
            (searcher_token, token_program_searcher)
            if swap_opportunity.fee_token == "searcher_token"
            else (user_token, token_program_user)
        )
        router = swap_opportunity.router_account

        return {
            "searcher_token": searcher_token,
            "token_program_searcher": token_program_searcher,
            "token_program_user": token_program_user,
            "user_token": user_token,
            "user": user,
            "mint_fee": mint_fee,
            "fee_token_program": fee_token_program,
            "router": router,
        }

    @staticmethod
    def get_svm_swap_instructions(
        searcher: Pubkey,
        bid_amount: int,
        deadline: int,
        chain_id: str,
        swap_opportunity: SwapOpportunitySvm,
        fee_receiver_relayer: Pubkey,
        relayer_signer: Pubkey,
    ) -> List[Instruction]:
        if chain_id not in SVM_CONFIGS:
            raise ValueError(f"Chain ID {chain_id} not supported")
        svm_config = SVM_CONFIGS[chain_id]
        program_id = svm_config["express_relay_program"]
        if (
            swap_opportunity.tokens.side_specified == "searcher"
            and swap_opportunity.fee_token == "user_token"
        ):
            # scale bid amount by FEE_SPLIT_PRECISION/(FEE_SPLIT_PRECISION-fees) to account for fees
            denominator = FEE_SPLIT_PRECISION - (
                swap_opportunity.platform_fee_bps + swap_opportunity.referral_fee_bps
            )
            numerator = bid_amount * FEE_SPLIT_PRECISION
            # add denominator - 1 to round up
            bid_amount = (numerator + (denominator - 1)) // denominator
        express_relay_metadata = LimoClient.get_express_relay_metadata_pda(program_id)
        fee_token = (
            swap_fee_token.Searcher()
            if swap_opportunity.fee_token == "searcher_token"
            else swap_fee_token.User()
        )
        amount_searcher = (
            swap_opportunity.tokens.searcher_amount
            if swap_opportunity.tokens.side_specified == "searcher"
            else bid_amount
        )
        amount_user = (
            swap_opportunity.tokens.user_amount_including_fees
            if swap_opportunity.tokens.side_specified == "user"
            else bid_amount
        )
        accs = ExpressRelayClient.extract_swap_info(swap_opportunity)

        instructions: List[Instruction] = []

        if swap_opportunity.memo is not None:
            instructions.append(
                Instruction(
                    program_id=MEMO_PROGRAM_ID,
                    accounts=[],
                    data=swap_opportunity.memo.encode(),
                )
            )

        token_accounts_to_create = ExpressRelayClient.get_token_accounts_to_create(
            searcher=searcher,
            swap_opportunity=swap_opportunity,
            fee_receiver_relayer=fee_receiver_relayer,
            express_relay_metadata=express_relay_metadata,
            configs=swap_opportunity.token_account_initialization_configs,
        )

        for token_account in token_accounts_to_create:
            instructions.append(
                create_associated_token_account_idempotent(
                    payer=token_account["payer"],
                    owner=token_account["owner"],
                    mint=token_account["mint"],
                    token_program_id=token_account["program"],
                )
            )

        if accs["user_token"] == WRAPPED_SOL_MINT:
            amount_to_wrap_user = ExpressRelayClient.get_user_amount_to_wrap(
                amount_user=amount_user,
                user_mint_user_balance=swap_opportunity.user_mint_user_balance,
                token_account_initialization_configs=swap_opportunity.token_account_initialization_configs,
            )
            instructions.extend(
                wrap_sol(searcher, accs["user"], amount_to_wrap_user, create_ata=False)
            )
        swap_ix = swap(
            {
                "data": SwapArgs(
                    deadline=deadline,
                    amount_searcher=amount_searcher,
                    amount_user=amount_user,
                    fee_token=fee_token,
                    referral_fee_bps=swap_opportunity.referral_fee_bps,
                )
            },
            {
                "searcher": searcher,
                "user": swap_opportunity.user_wallet_address,
                "searcher_ta_mint_searcher": get_ata(
                    searcher, accs["searcher_token"], accs["token_program_searcher"]
                ),
                "searcher_ta_mint_user": get_ata(
                    searcher, accs["user_token"], accs["token_program_user"]
                ),
                "user_ata_mint_searcher": get_ata(
                    swap_opportunity.user_wallet_address,
                    accs["searcher_token"],
                    accs["token_program_searcher"],
                ),
                "user_ata_mint_user": get_ata(
                    swap_opportunity.user_wallet_address,
                    accs["user_token"],
                    accs["token_program_user"],
                ),
                "router_fee_receiver_ta": get_ata(
                    accs["router"], accs["mint_fee"], accs["fee_token_program"]
                ),
                "relayer_fee_receiver_ata": get_ata(
                    fee_receiver_relayer, accs["mint_fee"], accs["fee_token_program"]
                ),
                "express_relay_fee_receiver_ata": get_ata(
                    express_relay_metadata, accs["mint_fee"], accs["fee_token_program"]
                ),
                "mint_searcher": accs["searcher_token"],
                "mint_user": accs["user_token"],
                "mint_fee": accs["mint_fee"],
                "token_program_searcher": accs["token_program_searcher"],
                "token_program_user": accs["token_program_user"],
                "token_program_fee": accs["fee_token_program"],
                "express_relay_metadata": express_relay_metadata,
                "relayer_signer": relayer_signer,
            },
            svm_config["express_relay_program"],
        )
        instructions.append(swap_ix)
        if (
            accs["searcher_token"] == WRAPPED_SOL_MINT
            or accs["user_token"] == WRAPPED_SOL_MINT
        ):
            instructions.append(unwrap_sol(accs["user"]))
        return instructions
