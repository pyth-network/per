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
        f.write(f'export DELETE_ENABLED=true\n')
        f.write(f'export DELETE_INTERVAL_SECONDS={1}\n')
        f.write(f'export DELETE_THRESHOLD_SECONDS={60*60*24*2}\n')

    mint_buy = Keypair.from_json((open('keypairs/mint_buy.json').read())).pubkey()
    mint_sell = Keypair.from_json((open('keypairs/mint_sell.json').read())).pubkey()
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
    auction_time: 250ms
    token_whitelist:
      enabled: true
      whitelist_mints:
        - {mint_buy}
        - {mint_sell}
        - So11111111111111111111111111111111111111112
    allow_permissionless_quote_requests: true
    minimum_fee_list:
      profiles:
        - profile_id: 4b4f8bcf-415a-4509-be21-bd803cdc8937
          minimum_fees:
            - mint: {mint_buy}
              fee_ppm: 200
            - mint: {mint_sell}
              fee_ppm: 0
lazer:
  price_feeds:
    - id: 1
      mint: {mint_buy}
      exponent: -8
    - id: 2
      mint: {mint_sell}
      exponent: -8
'''
    with open('auction-server/config.yaml', 'w') as f:
        f.write(template)


if __name__ == '__main__':
    main()
