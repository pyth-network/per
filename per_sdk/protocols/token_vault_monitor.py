import argparse
import asyncio
import base64
import json
import logging
import urllib.parse
from typing import TypedDict

import httpx
import web3
from eth_abi import encode

from per_sdk.utils.pyth_prices import PriceFeed, PriceFeedClient
from per_sdk.utils.types_liquidation_adapter import LiquidationOpportunity

logger = logging.getLogger(__name__)


class ProtocolAccount(TypedDict):
    """
    ProtocolAccount is a TypedDict that represents an account/vault in the protocol.

    This class contains all the relevant information about a vault/account on this protocol that is necessary for identifying whether it is eligible for liquidation and constructing a LiquidationOpportunity object.
    """

    account_number: int
    token_address_collateral: str
    token_address_debt: str
    token_id_collateral: str
    token_id_debt: str
    amount_collateral: int
    amount_debt: int
    min_health_ratio: int
    min_permissionless_health_ratio: int


def get_vault_abi():
    f = open("per_multicall/out/TokenVault.sol/TokenVault.json")

    data = json.load(f)

    return data["abi"]


class VaultMonitor:
    def __init__(
        self, rpc_url: str, contract_address: str, weth_address: str, chain_id: str
    ):
        self.rpc_url = rpc_url
        self.contract_address = contract_address
        self.weth_address = weth_address
        self.chain_id = chain_id
        self.w3 = web3.AsyncWeb3(web3.AsyncHTTPProvider(rpc_url))

        self.token_vault = self.w3.eth.contract(
            address=contract_address, abi=get_vault_abi()
        )
        self.price_feed_client = PriceFeedClient([])

    async def get_accounts(self) -> list[ProtocolAccount]:
        """
        Returns all the open accounts in the protocol in the form of a list of type ProtocolAccount.

        Args:
            rpc_url (str): The RPC URL of the chain
        Returns:
            List of objects of type ProtocolAccount (defined above). Each ProtocolAccount object represents an account/vault in the protocol.
        """
        abi = get_vault_abi()

        j = 0
        while abi[j].get("name") != "getVault":
            j += 1
        get_vault_details = abi[j]["outputs"][0]["components"]
        vault_struct = [x["name"] for x in get_vault_details]

        accounts = []
        done = False
        account_number = 0

        while not done:
            vault = await self.token_vault.functions.getVault(account_number).call()
            vault_dict = dict(zip(vault_struct, vault))

            if int(vault_dict["tokenCollateral"], 16) == 0:  # vault is not created yet
                done = True
            else:
                account: ProtocolAccount = {
                    "account_number": account_number,
                    "token_address_collateral": vault_dict["tokenCollateral"],
                    "token_id_collateral": vault_dict["tokenIDCollateral"].hex(),
                    "token_address_debt": vault_dict["tokenDebt"],
                    "token_id_debt": vault_dict["tokenIDDebt"].hex(),
                    "amount_collateral": vault_dict["amountCollateral"],
                    "amount_debt": vault_dict["amountDebt"],
                    "min_health_ratio": vault_dict["minHealthRatio"],
                    "min_permissionless_health_ratio": vault_dict[
                        "minPermissionLessHealthRatio"
                    ],
                }
                accounts.append(account)
                account_number += 1

        return accounts

    def create_liquidation_opp(
        self, account: ProtocolAccount, prices: list[PriceFeed]
    ) -> LiquidationOpportunity:
        """
        Constructs a LiquidationOpportunity object from a ProtocolAccount object and a set of relevant Pyth PriceFeeds.

        Args:
            account: A ProtocolAccount object, representing an account/vault in the protocol.
            prices: A list of PriceFeed objects, representing the relevant Pyth price feeds for the tokens in the ProtocolAccount object.
        Returns:
            A LiquidationOpportunity object corresponding to the specified account.
        """

        price_updates = [base64.b64decode(update["vaa"]) for update in prices]

        calldata = self.token_vault.encodeABI(
            fn_name="liquidateWithPriceUpdate",
            args=[account["account_number"], price_updates],
        )
        permission_payload = encode(["uint256"], [account["account_number"]])
        permission = (
            "0x"
            + encode(
                ["address", "bytes"], [self.contract_address, permission_payload]
            ).hex()
        )
        call_value = len(price_updates)

        if call_value > 0 and account["token_address_collateral"] == self.weth_address:
            repay_tokens = [
                (
                    account["token_address_debt"],
                    str(account["amount_debt"] + call_value),
                )
            ]
        else:
            repay_tokens = [
                (account["token_address_debt"], str(account["amount_debt"])),
                (self.weth_address, str(call_value)),
            ]

        opp: LiquidationOpportunity = {
            "chain_id": self.chain_id,
            "contract": self.contract_address,
            "calldata": calldata,
            "permission_key": permission,
            "account": str(account["account_number"]),
            "value": str(call_value),
            "repay_tokens": repay_tokens,
            "receipt_tokens": [
                (account["token_address_collateral"], str(account["amount_collateral"]))
            ],
            "version": "v1",
        }

        # TODO: figure out best interface to show partial liquidation possibility? Is this even important?
        # NOTE: the above interface may work out fine for single collateral,
        # single debt vaults, since most of them just have proportional (linear)
        # liquidation amount functions. But may not work well for multi-asset
        # vaults bc users may need to do out the price calculations themselves.

        return opp

    async def get_liquidation_opportunities(self) -> list[LiquidationOpportunity]:
        """
        Filters list of ProtocolAccount types to return a list of LiquidationOpportunity types.

        Args:
            accounts: A list of ProtocolAccount objects, representing all the open accounts in the protocol.
            prices: A dictionary of Pyth price feeds, where the keys are Pyth feed IDs and the values are PriceFeed objects.
        Returns:
            A list of LiquidationOpportunity objects, one per account that is eligible for liquidation.
        """

        liquidatable = []
        accounts = await self.get_accounts()
        for account in accounts:
            # vault is already liquidated
            if account["amount_collateral"] == 0 and account["amount_debt"] == 0:
                continue
            # TODO: optimize this to only query for the price feeds that are needed and only query once
            (
                price_collateral,
                price_debt,
            ) = await self.price_feed_client.get_pyth_prices_latest(
                [account["token_id_collateral"], account["token_id_debt"]]
            )
            if price_collateral is None:
                raise Exception(
                    f"Price for collateral token {account['token_id_collateral']} not found"
                )

            if price_debt is None:
                raise Exception(
                    f"Price for debt token {account['token_id_debt']} not found"
                )

            value_collateral = (
                int(price_collateral["price"]["price"]) * account["amount_collateral"]
            )
            value_debt = int(price_debt["price"]["price"]) * account["amount_debt"]
            health = value_collateral / value_debt
            logger.debug(f"Account {account['account_number']} health: {health}")
            if (
                value_debt * int(account["min_health_ratio"])
                > value_collateral * 10**18
            ):
                price_updates = [price_collateral, price_debt]
                liquidatable.append(self.create_liquidation_opp(account, price_updates))

        return liquidatable


async def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("-v", "--verbose", action="count", default=0)
    parser.add_argument(
        "--chain-id",
        type=str,
        required=True,
        help="Chain ID of the network to monitor for liquidation opportunities",
    )
    parser.add_argument(
        "--rpc-url",
        type=str,
        required=True,
        help="Chain RPC endpoint, used to fetch on-chain data via get_accounts",
    )
    parser.add_argument(
        "--vault-contract",
        type=str,
        required=True,
        dest="vault_contract",
        help="Token vault contract address",
    )
    parser.add_argument(
        "--weth-contract",
        type=str,
        required=True,
        dest="weth_contract",
        help="WETH contract address",
    )
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument(
        "--dry-run",
        action="store_false",
        dest="broadcast",
        help="If provided, will not send liquidation opportunities to the server",
    )
    group.add_argument(
        "--liquidation-server-url",
        type=str,
        help="Liquidation server endpoint; if provided, will send liquidation opportunities to this endpoint",
    )
    args = parser.parse_args()

    logger.setLevel(logging.INFO if args.verbose == 0 else logging.DEBUG)
    log_handler = logging.StreamHandler()
    formatter = logging.Formatter(
        "%(asctime)s %(levelname)s:%(name)s:%(module)s %(message)s",
        datefmt="%Y-%m-%d %H:%M:%S",
    )
    log_handler.setFormatter(formatter)
    logger.addHandler(log_handler)

    monitor = VaultMonitor(
        args.rpc_url, args.vault_contract, args.weth_contract, args.chain_id
    )

    while True:
        opportunities = await monitor.get_liquidation_opportunities()

        if args.broadcast:
            client = httpx.AsyncClient()
            for opp in opportunities:
                try:
                    resp = await client.post(
                        urllib.parse.urljoin(
                            args.liquidation_server_url, "/v1/liquidation/opportunities"
                        ),
                        json=opp,
                    )
                except Exception as e:
                    logger.error(f"Failed to post to liquidation server: {e}")
                    await asyncio.sleep(1)
                    continue
                if resp.status_code == 200:
                    logger.info("Successfully broadcasted the opportunity")
                else:
                    logger.error(
                        f"Failed to post to liquidation server, status code: {resp.status_code}, response: {resp.text}"
                    )

        else:
            logger.info(
                f"List of liquidatable accounts:\n{json.dumps(opportunities, indent=2)}"
            )

        await asyncio.sleep(1)


if __name__ == "__main__":
    asyncio.run(main())
