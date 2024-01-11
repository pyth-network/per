import web3
from web3.auto import w3
from eth_abi import encode


def construct_signature_liquidator(
    repay_tokens: list[(str, int)],
    receipt_tokens: list[(str, int)],
    address: str,
    liq_calldata: bytes,
    bid: int,
    valid_until: int,
    secret_key: str
):
    digest = encode(
        ['(address,uint256)[]', '(address,uint256)[]',
         'address', 'bytes', 'uint256'],
        [repay_tokens, receipt_tokens, address, liq_calldata, bid]
    )
    msg_data = web3.Web3.solidity_keccak(
        ['bytes', 'uint256'], [digest, valid_until])
    signature = w3.eth.account.signHash(
        msg_data, private_key=secret_key)

    return signature
