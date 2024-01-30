import web3
from eth_abi import encode
import json
from typing import TypedDict
import argparse
import logging
import asyncio
import httpx

from beacon.utils.pyth_prices import PriceFeedClient, PriceFeed
from beacon.utils.types_liquidation_adapter import LiquidationOpportunity

TOKEN_VAULT_ADDRESS = "0x72A22FfcAfa6684d4EE449620270ac05afE963d0"


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
    f = open('per_multicall/out/TokenVault.sol/TokenVault.json')

    data = json.load(f)

    return data['abi']


async def get_accounts(rpc_url: str) -> list[ProtocolAccount]:
    """
    Returns all the open accounts in the protocol in the form of a list of type ProtocolAccount.

    Args:
        rpc_url (str): The RPC URL of the chain
    Returns:
        List of objects of type ProtocolAccount (defined above). Each ProtocolAccount object represents an account/vault in the protocol.
    """
    abi = get_vault_abi()
    w3 = web3.AsyncWeb3(web3.AsyncHTTPProvider(rpc_url))
    token_vault = w3.eth.contract(
        address=TOKEN_VAULT_ADDRESS,
        abi=abi)

    j = 0
    while abi[j].get('name') != 'getVault':
        j += 1
    get_vault_details = abi[j]['outputs'][0]['components']
    vault_struct = [x['name'] for x in get_vault_details]

    accounts = []
    done = False
    account_number = 0

    while not done:
        vault = await token_vault.functions.getVault(account_number).call()
        vault_dict = dict(zip(vault_struct, vault))

        if int(
                vault_dict['tokenCollateral'],
                16) == 0 and int(
                vault_dict['tokenDebt'],
                16) == 0:
            done = True
        else:
            account: ProtocolAccount = {
                "account_number": account_number,
                "token_address_collateral": vault_dict['tokenCollateral'],
                "token_id_collateral": vault_dict['tokenIDCollateral'].hex(),
                "token_address_debt": vault_dict['tokenDebt'],
                "token_id_debt": vault_dict['tokenIDDebt'].hex(),
                "amount_collateral": vault_dict['amountCollateral'],
                "amount_debt": vault_dict['amountDebt'],
                "min_health_ratio": vault_dict['minHealthRatio'],
                "min_permissionless_health_ratio": vault_dict['minPermissionLessHealthRatio']}
            accounts.append(account)
            account_number += 1

    return accounts


def create_liquidation_opp(
        account: ProtocolAccount,
        prices: list[PriceFeed]) -> LiquidationOpportunity:
    """
    Constructs a LiquidationOpportunity object from a ProtocolAccount object and a set of relevant Pyth PriceFeeds.

    Args:
        account: A ProtocolAccount object, representing an account/vault in the protocol.
        prices: A list of PriceFeed objects, representing the relevant Pyth price feeds for the tokens in the ProtocolAccount object.
    Returns:
        A LiquidationOpportunity object corresponding to the specified account.
    """

    # [bytes.fromhex(update['vaa']) for update in prices] ## TODO: uncomment this, to add back price updates
    price_updates = []
    function_signature = web3.Web3.solidity_keccak(
        ["string"], ["liquidateWithPriceUpdate(uint256,bytes[])"]
    )[:4].hex()
    calldata = (
        function_signature
        + encode(
            ["uint256", "bytes[]"], [account["account_number"], price_updates]
        ).hex()
    )

    msg = encode(["uint256"], [account["account_number"]])
    permission = "0x" + encode(["address", "bytes"],
                               [TOKEN_VAULT_ADDRESS, msg]).hex()

    opp: LiquidationOpportunity = {
        "chain_id": "development",
        "contract": TOKEN_VAULT_ADDRESS,
        "calldata": calldata,
        "permission_key": permission,
        "account": str(account["account_number"]),
        "repay_tokens": [
            {
                "contract": account["token_address_debt"],
                "amount": str(account["amount_debt"]),
            }
        ],
        "receipt_tokens": [
            {
                "contract": account["token_address_collateral"],
                "amount": str(account["amount_collateral"]),
            }
        ]
    }

    # TODO: figure out best interface to show partial liquidation possibility? Is this even important?
    # NOTE: the above interface may work out fine for single collateral,
    # single debt vaults, since most of them just have proportional (linear)
    # liquidation amount functions. But may not work well for multi-asset
    # vaults bc users may need to do out the price calculations themselves.

    return opp


def get_liquidatable(accounts: list[ProtocolAccount],
                     prices: dict[str,
                                  PriceFeed]) -> (list[LiquidationOpportunity]):
    """
    Filters list of ProtocolAccount types to return a list of LiquidationOpportunity types.

    Args:
        accounts: A list of ProtocolAccount objects, representing all the open accounts in the protocol.
        prices: A dictionary of Pyth price feeds, where the keys are Pyth feed IDs and the values are PriceFeed objects.
    Returns:
        A list of LiquidationOpportunity objects, one per account that is eligible for liquidation.
    """

    liquidatable = []

    for account in accounts:
        price_collateral = prices.get(account["token_id_collateral"])
        if price_collateral is None:
            raise Exception(
                f"Price for collateral token {account['token_id_collateral']} not found")

        price_debt = prices.get(account["token_id_debt"])
        if price_debt is None:
            raise Exception(
                f"Price for debt token {account['token_id_debt']} not found")

        value_collateral = (
            int(price_collateral["price"]["price"]) *
            account["amount_collateral"]
        )
        value_debt = int(price_debt["price"]["price"]) * account["amount_debt"]

        if value_debt * int(account["min_health_ratio"]) > value_collateral * 10**18:
            print("unhealthy vault")
            print(
                value_debt * int(account["min_health_ratio"]),
                value_collateral * 10**18,
            )
            price_updates = [price_collateral, price_debt]
            liquidatable.append(create_liquidation_opp(account, price_updates))

    return liquidatable


async def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--operator-api-key", type=str, required=True,
                        help="Operator API key, used to authenticate the surface post request")
    parser.add_argument("--rpc-url", type=str, required=True,
                        help="Chain RPC endpoint, used to fetch on-chain data via get_accounts")
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument("--dry-run", action="store_false", dest="send_beacon",
                       help="If provided, will not send liquidation opportunities to the beacon server")
    group.add_argument("--beacon-server-url", type=str,
                       help="Beacon server endpoint; if provided, will send liquidation opportunities to the beacon server")

    parser.add_argument("--log-file", type=str,
                        help="Path of log file where to save log statements; if not provided, will print to stdout")
    args = parser.parse_args()

    if args.log_file:
        logging.basicConfig(filename=args.log_file, level=logging.INFO)
    else:
        logging.basicConfig(level=logging.INFO)

    feed_ids = ["ff61491a931112ddf1bd8147cd1b641375f79f5825126d665480874634fd0ace",
                "e62df6c8b4a85fe1a67db44dc12de5db330f7ac66b72dc658afedf0f4a415b43"]  # TODO: should this be automated rather than hardcoded?
    price_feed_client = PriceFeedClient(feed_ids)

    # TODO: sometimes the ws doesn't pull prices, understand why
    ws_call = price_feed_client.ws_pyth_prices()
    asyncio.create_task(ws_call)

    client = httpx.AsyncClient()

    await asyncio.sleep(2)

    while True:
        accounts = await get_accounts(args.rpc_url)

        accounts_liquidatable = get_liquidatable(
            accounts, price_feed_client.prices_dict)

        if args.send_beacon:
            resp = await client.post(
                args.beacon_server_url,
                json=accounts_liquidatable
            )
            if resp.status_code == 422:
                logging.error(
                    "Invalid request body format, should provide a list of LiquidationOpportunity")
            elif resp.status_code == 404:
                logging.error("Provided beacon server endpoint url not found")
            elif resp.status_code == 405:
                logging.error(
                    "Provided beacon server endpoint url does not support POST requests")
            else:
                logging.info(f"Response, post to beacon: {resp.text}")
        else:
            logging.info(
                f"List of liquidatable accounts:\n{accounts_liquidatable}")

        await asyncio.sleep(2)

if __name__ == "__main__":
    asyncio.run(main())
