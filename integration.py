"""
This script is used to generate the environment variables to be used by python scripts
and config.yaml file for the auction server.

It accepts the geth rpc address as the first argument and generates the following:
1. tilt-resources.env file which contains the environment variables to be used by the python scripts
2. config.yaml file which contains the configuration for the auction server
"""

import json
import sys

field_mapping = {
    'tokenVault': 'TOKEN_VAULT',
    'weth': 'WETH',
    'relayerPrivateKey': 'RELAYER_PRIVATE_KEY',
    'searcherAOwnerSk': 'SEARCHER_SK',
    'adapterFactory': 'ADAPTER_FACTORY',
    'adapterBytecodeHash': 'ADAPTER_BYTECODE_HASH',
}


def main():
    latest_env = json.load(open('contracts/latestEnvironment.json'))
    with open('tilt-resources.env', 'w') as f:
        for k, v in field_mapping.items():
            f.write(f'export {v}={latest_env[k]}\n')
        f.write('export SECRET_KEY=admin\n')
    # config_template
    template = f'''
chains:
  development:
    geth_rpc_addr: {sys.argv[1]}
    geth_ws_addr: {sys.argv[2]}
    rpc_timeout: 2
    express_relay_contract: {latest_env['expressRelay']}
    adapter_factory_contract: {latest_env['adapterFactory']}
    permit2_contract: {latest_env['permit2']}
    legacy_tx: false
    poll_interval: 1
'''
    with open('auction-server/config.yaml', 'w') as f:
        f.write(template)


if __name__ == '__main__':
    main()
