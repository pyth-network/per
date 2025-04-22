"""
This script is used to generate the environment variables to be used by python scripts
and config.yaml file for the auction server.

1. tilt-resources.env file which contains the environment variables to be used by the python scripts
2. config.yaml file which contains the configuration for the auction server
"""

from solders.keypair import Keypair

def main():
    relayer_key_svm = Keypair.from_json((open('keypairs/relayer_signer.json').read()))
    with open('tilt-resources.env', 'w') as f:
        f.write('export SECRET_KEY=admin\n')
        f.write(f'export PRIVATE_KEY_SVM={str(relayer_key_svm)}\n')
    # config_template
    # Added two rpc_tx_submission_urls for test
    template = f'''
chains:
  local-solana:
    express_relay_program_id: PytERJFhAKuNNuaiXkApLfWzwNwSNDACpigT3LwQfou
    rpc_read_url: http://localhost:8899
    rpc_tx_submission_urls:
      - http://localhost:8899
      - http://localhost:8899
    ws_addr: ws://localhost:8900
    accepted_token_programs:
      - TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA
      - TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb
    ordered_fee_tokens: []
    jupiter_ultra_url: https://ultra-api.jup.ag
'''
    with open('auction-server/config.yaml', 'w') as f:
        f.write(template)


if __name__ == '__main__':
    main()
