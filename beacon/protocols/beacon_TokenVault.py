import web3
from eth_abi import encode
import json
from typing import TypedDict
import argparse

from beacon.utils.pyth_prices import *
from beacon.utils.types_liquidation_adapter import *

TOKEN_VAULT_ADDRESS = "0x72A22FfcAfa6684d4EE449620270ac05afE963d0"


class LiquidationAccount(TypedDict):
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



async def get_accounts(rpc_url: str) -> list[LiquidationAccount]:
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
            account: LiquidationAccount = {
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
        account: LiquidationAccount,
        prices: list[PriceFeed]) -> LiquidationOpportunity:
    # [bytes.fromhex(update['vaa']) for update in prices] ## TODO: uncomment this, to add back price updates
    price_updates = []
    function_signature = web3.Web3.solidity_keccak(
        ["string"], ["liquidateWithPriceUpdate(uint256,bytes[])"])[:4].hex()
    calldata = function_signature + \
        encode(['uint256', 'bytes[]'], [
               account["account_number"], price_updates]).hex()

    msg = encode(["uint256"], [account["account_number"]])
    permission = '0x' + \
        encode(['address', 'bytes'], [TOKEN_VAULT_ADDRESS, msg]).hex()

    opp: LiquidationOpportunity = {
        "chain_id": "development",
        "contract": TOKEN_VAULT_ADDRESS,
        "calldata": calldata,
        "permission_key": permission,
        "account": str(account["account_number"]),
        "repay_tokens": [
            (
                account["token_address_debt"],
                hex(account["amount_debt"])
            )
        ],
        "receipt_tokens": [
            (
                account["token_address_collateral"],
                hex(account["amount_collateral"])
            )
        ],
        "prices": price_updates,
    }

    # TODO: figure out best interface to show partial liquidation possibility? Is this even important?
    # NOTE: the above interface may work out fine for single collateral,
    # single debt vaults, since most of them just have proportional (linear)
    # liquidation amount functions. But may not work well for multi-asset
    # vaults bc users may need to do out the price calculations themselves.

    return opp



def get_liquidatable(accounts: list[LiquidationAccount],
                     prices: dict[str,
                                  PriceFeed]) -> (list[LiquidationOpportunity]):
    liquidatable = []

    for account in accounts:
        price_collateral = prices[account["token_id_collateral"]]
        price_debt = prices[account["token_id_debt"]]

        value_collateral = int(
            price_collateral['price']['price']) * account["amount_collateral"]
        value_debt = int(price_debt['price']['price']) * account["amount_debt"]

        if value_debt * int(account["min_health_ratio"]) > value_collateral * 10**18:
            price_updates = [price_collateral, price_debt]
            liquidatable.append(
                create_liquidation_opp(
                    account, price_updates))

    return liquidatable


async def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--operator_api_key", type=str, required=True, help="Operator API key, used to authenticate the surface post request")
    parser.add_argument("--rpc_url", type=str, required=True, help="Chain RPC endpoint, used to fetch on-chain data via get_accounts")
    parser.add_argument("--beacon_server_url", type=str, help="Beacon server endpoint; if provided, will send liquidation opportunities to the beacon server; otherwise, will just print them out")
    args = parser.parse_args()

    # get prices
    feed_ids = ["ff61491a931112ddf1bd8147cd1b641375f79f5825126d665480874634fd0ace", "e62df6c8b4a85fe1a67db44dc12de5db330f7ac66b72dc658afedf0f4a415b43"] # TODO: should this be automated rather than hardcoded?
    price_feed_client = PriceFeedClient(feed_ids)

    ws_call = price_feed_client.ws_pyth_prices()
    task = asyncio.create_task(ws_call)

    client = httpx.AsyncClient()

    await asyncio.sleep(2)

    while True:
        # get all accounts
        accounts = await get_accounts(args.rpc_url)

        liquidatable = get_liquidatable(accounts, price_feed_client.prices_dict)

        if args.beacon_server_url:
            resp = await client.post(
                args.beacon_server_url,
                json=liquidatable
            )
            print(f"Response, post to beacon: {resp.text}")
        else:
            print(liquidatable)
        await asyncio.sleep(2)

if __name__ == "__main__":
    asyncio.run(main())
