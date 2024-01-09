import web3
from web3.auto import w3
from eth_account import Account
from eth_account.signers.local import LocalAccount
from eth_abi import encode
import httpx

from pythresearch.per.beacon.utils.types_liquidation_adapter import *
from pythresearch.per.beacon.utils.endpoints import *

TOKEN_VAULT_ADDRESS = "0x72A22FfcAfa6684d4EE449620270ac05afE963d0"

def create_liquidation_intent(
        opp: LiquidationOpportunity,
        sk_liquidator: str) -> LiquidationAdapterIntent:
    repay_tokens = [(opp['repay_tokens'][0][0], opp['repay_tokens'][0][2])]
    receipt_tokens = [
        (opp['receipt_tokens'][0][0],
         opp['receipt_tokens'][0][2])]
    bid = 10
    valid_until = 1_000_000_000_000

    account: LocalAccount = Account.from_key(sk_liquidator)
    liquidator = account.address
    liq_calldata = bytes.fromhex(
        opp['data'][2:]) if opp['data'][:2] == "0x" else bytes.fromhex(opp['data'])
    digest = encode(
        ['(address,uint256)[]', '(address,uint256)[]', 'address', 'bytes', 'uint256'],
        [repay_tokens, receipt_tokens, opp['contract'], liq_calldata, bid]
    )
    msg_data = web3.Web3.solidity_keccak(
        ['bytes', 'uint256'], [digest, valid_until])
    signature_liquidator = w3.eth.account.signHash(
        msg_data, private_key=sk_liquidator)

    liquidation_params_types = '((address,uint256)[],(address,uint256)[],address,address,bytes,uint256,uint256,bytes)'

    fn_signature = web3.Web3.solidity_keccak(
        ["string"], [f"callLiquidation({liquidation_params_types})"])[:4].hex()
    liquidation_params = (
        repay_tokens,
        receipt_tokens,
        liquidator,
        opp['contract'],
        liq_calldata,
        valid_until,
        bid,
        signature_liquidator.signature)
    calldata = fn_signature + encode(
        [liquidation_params_types],
        [liquidation_params]
    ).hex()

    intent: LiquidationAdapterIntent = {
        "bid": hex(bid),
        "calldata": calldata,
        "chain_id": "development",
        "contract": LIQUIDATION_ADAPTER_ADDRESS,
        "permission_key": opp['permission']
    }

    return intent


async def main():
    CLIENT = httpx.AsyncClient()

    params = {"contract": TOKEN_VAULT_ADDRESS}

    # TODO: get the liquidatable vaults from the endpoint
    liquidatable_permissionless, liquidatable_per = await CLIENT.get(BEACON_SERVER_ENDPOINT, params=params)

    # this is hardcoded to the searcher A SK
    sk_liquidator = "0x5b1efe5da513271c0d30cde7a2ad1d29456d68abd592efdaa7d2302e913b783f"
    intent = create_liquidation_intent(liquidatable_per[0], sk_liquidator)

    resp = await CLIENT.post(
        AUCTION_SERVER_ENDPOINT,
        json=intent
    )

    print(resp.text)

    import pdb
    pdb.set_trace()