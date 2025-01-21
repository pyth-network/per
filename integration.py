"""
This script is used to generate the environment variables to be used by python scripts
and config.yaml file for the auction server.

It accepts the geth rpc address as the first argument and generates the following:
1. tilt-resources.env file which contains the environment variables to be used by the python scripts
2. config.yaml file which contains the configuration for the auction server
"""

import json
import sys

from solders.keypair import Keypair

field_mapping = {
    'tokenVault': 'TOKEN_VAULT',
    'weth': 'WETH',
    'relayerPrivateKey': 'RELAYER_PRIVATE_KEY',
    'searcherAOwnerSk': 'SEARCHER_SK',
    'adapterFactory': 'ADAPTER_FACTORY',
    'adapterBytecodeHash': 'ADAPTER_BYTECODE_HASH',
    'permit2': 'PERMIT2',
    'chainIdNum': 'CHAIN_ID_NUM',
}

def main():
    latest_env = json.load(open('contracts/evm/latestEnvironment.json'))
    relayer_key_svm = Keypair.from_json((open('keypairs/relayer_signer.json').read()))
    with open('tilt-resources.env', 'w') as f:
        for k, v in field_mapping.items():
            f.write(f'export {v}={latest_env[k]}\n')
        f.write('export SECRET_KEY=admin\n')
        f.write(f'export PRIVATE_KEY_SVM={str(relayer_key_svm)}\n')
    # config_template
    template = f'''
chains:
  development:
    geth_rpc_addr: {sys.argv[1]}
    geth_ws_addr: {sys.argv[2]}
    rpc_timeout: 2
    express_relay_contract: {latest_env['expressRelay']}
    adapter_factory_contract: {latest_env['adapterFactory']}
    legacy_tx: false
    poll_interval: 1
  local-solana:
    express_relay_program_id: PytERJFhAKuNNuaiXkApLfWzwNwSNDACpigT3LwQfou
    rpc_read_url: http://localhost:8899
    rpc_tx_submission_url: http://localhost:8899
    ws_addr: ws://localhost:8900
    accepted_token_programs:
      - TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA
      - TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb
'''
    with open('auction-server/config.yaml', 'w') as f:
        f.write(template)


if __name__ == '__main__':
    main()
