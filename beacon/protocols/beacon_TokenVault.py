import web3
from eth_abi import encode
import json
from typing import TypedDict

from pythresearch.per.beacon.utils.pyth_prices import *
from pythresearch.per.beacon.utils.types_liquidation_adapter import *

TOKEN_VAULT_ADDRESS = "0x72A22FfcAfa6684d4EE449620270ac05afE963d0"
CHAIN_RPC_ENDPOINT = "http://localhost:8545"


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
    f = open('pythresearch/per/per_multicall/out/TokenVault.sol/TokenVault.json')

    data = json.load(f)

    return data['abi']


"""
get_accounts() is the first method that the protocol should implement. It should take no arguments and return all the open accounts in the protocol in the form of a list of objects of type LiquidationAccount (defined above). Each LiquidationAccount object represents an account/vault in the protocol.
This function can be implemented in any way, but it should be able to return all the open accounts in the protocol. For some protocols, this may be easily doable by just querying on-chain state; however, most protocols will likely need to maintain or access an off-chain indexer to get the list of all open accounts.
"""


async def get_accounts() -> list[LiquidationAccount]:
    abi = get_vault_abi()
    w3 = web3.AsyncWeb3(web3.AsyncHTTPProvider(CHAIN_RPC_ENDPOINT))
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


"""
get_liquidatable(accounts, prices) is the second method that the protocol should implement. It should take two arguments: account--a list of Account (defined above) objects--and prices--a dictionary of Pyth prices.
accounts should be the list of all open accounts in the protocol (i.e. the output of get_accounts()).
prices should be a dictionary of Pyth prices, where the keys are Pyth feed IDs and the values are PriceFeed objects. prices can be retrieved from the provided price retrieval functions.
This function should return a lists of liquidation opportunities. Each opportunity should be of the form LiquidationOpportunity defined above.
"""


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
    # get all accounts
    accounts = await get_accounts()

    # get prices
    pyth_price_feed_ids = await get_price_feed_ids()
    pyth_prices_latest = []
    i = 0
    cntr = 100
    while len(pyth_price_feed_ids[i:i + cntr]) > 0:
        pyth_prices_latest += await get_pyth_prices_latest(pyth_price_feed_ids[i:i + cntr])
        i += cntr
    pyth_prices_latest = dict(pyth_prices_latest)

    # get liquidatable accounts
    liquidatable = get_liquidatable(accounts, pyth_prices_latest)

    print(liquidatable)

if __name__ == "__main__":
    asyncio.run(main())
