load("ext://uibutton", "cmd_button", "location", "text_input")


rpc_port_anvil = "9545"
rpc_url_anvil = "http://127.0.0.1:%s" % rpc_port_anvil
ws_url_anvil = "ws://127.0.0.1:%s" % rpc_port_anvil

rpc_port_solana = "8899"
rpc_port_solana_ws = "8900"
rpc_url_solana = "http://127.0.0.1:%s" % rpc_port_solana
ws_url_solana = "ws://127.0.0.1:%s" % rpc_port_solana_ws

# Default anvil private key
private_key = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
block_time = "2"

# evm resources
local_resource(
    "evm-anvil",
    serve_cmd="anvil --gas-limit 500000000000000000 --block-time %s -p %s"
    % (block_time, rpc_port_anvil),
    readiness_probe=probe(
        period_secs=5,
        exec=exec_action(
            ["cast", "cid", "--rpc-url", rpc_url_anvil]
        ),  # get chain id as a readiness probe
    ),
)

forge_base_command = (
    "forge script script/Vault.s.sol --via-ir --private-key $PRIVATE_KEY --fork-url %s -vvv"
    % rpc_url_anvil
)

# we set automine to true before deployment and then set the interval to the block time after the deployment
# to speed up the deployment
local_resource(
    "evm-deploy-contracts",
    "cast rp --rpc-url http://localhost:9545 evm_setAutomine true; "
    + forge_base_command
    + " --sig 'setUpLocalnet()' --broadcast; "
    + "cast rp --rpc-url http://localhost:9545 evm_setIntervalMining %s" % block_time,
    dir="contracts/evm",
    env={"PRIVATE_KEY": private_key},
    resource_deps=["evm-anvil"],
)

cmd_button(
    "vault state",
    argv=[
        "sh",
        "-c",
        "cd contracts/evm; "
        + forge_base_command
        + " --sig 'getVault(uint256)' $VAULT --broadcast",
    ],
    location=location.NAV,
    resource="evm-deploy-contracts",
    env=["PRIVATE_KEY=" + private_key],
    icon_name="search",
    text="Get vault state",
    inputs=[
        text_input("VAULT", placeholder="Enter vault number"),
    ],
)

cmd_button(
    "create new vault",
    argv=[
        "sh",
        "-c",
        "cd contracts/evm; "
        + forge_base_command
        + " --sig 'createLiquidatableVault()' --broadcast",
    ],
    location=location.NAV,
    resource="evm-deploy-contracts",
    env=["PRIVATE_KEY=" + private_key],
    icon_name="add",
    text="Add Evm Opportunity",
)

local_resource(
    "create-server-configs", "poetry -C tilt-scripts run python3 integration.py %s %s" % (rpc_url_anvil, ws_url_anvil), resource_deps=["evm-deploy-contracts","svm-create-mints"]
)

local_resource(
    "svm-build-programs",
    "cargo build-sbf && anchor build",
    dir="contracts/svm",
)

# creates mints for sell and buy tokens, creates and funds ATAs for searcher and admin
local_resource(
    "svm-create-mints",
        """solana-keygen new -o keypairs/mint_buy.json -f --no-bip39-passphrase \
        && solana-keygen new -o keypairs/mint_sell.json -f --no-bip39-passphrase \
        && spl-token create-token -u localhost --fee-payer keypairs/admin.json --decimals 6 --mint-authority keypairs/admin.json keypairs/mint_sell.json \
        && spl-token create-token -u localhost --fee-payer keypairs/admin.json --decimals 6 --mint-authority keypairs/admin.json keypairs/mint_buy.json \
        && spl-token create-account -u localhost So11111111111111111111111111111111111111112 --fee-payer keypairs/admin.json --owner keypairs/searcher_rust.json \
        && spl-token create-account -u localhost So11111111111111111111111111111111111111112 --fee-payer keypairs/admin.json --owner keypairs/searcher_py.json \
        && spl-token create-account -u localhost So11111111111111111111111111111111111111112 --fee-payer keypairs/admin.json --owner keypairs/searcher_js.json \
        && spl-token create-account -u localhost keypairs/mint_buy.json --fee-payer keypairs/admin.json --owner keypairs/searcher_js.json \
        && spl-token create-account -u localhost keypairs/mint_sell.json --fee-payer keypairs/admin.json --owner keypairs/searcher_js.json \
        && spl-token create-account -u localhost keypairs/mint_buy.json --fee-payer keypairs/admin.json --owner keypairs/searcher_py.json \
        && spl-token create-account -u localhost keypairs/mint_sell.json --fee-payer keypairs/admin.json --owner keypairs/searcher_py.json \
        && spl-token create-account -u localhost keypairs/mint_buy.json --fee-payer keypairs/admin.json --owner keypairs/searcher_rust.json \
        && spl-token create-account -u localhost keypairs/mint_sell.json --fee-payer keypairs/admin.json --owner keypairs/searcher_rust.json \
        && spl-token create-account -u localhost keypairs/mint_buy.json --fee-payer keypairs/admin.json --owner keypairs/admin.json \
        && spl-token create-account -u localhost keypairs/mint_sell.json --fee-payer keypairs/admin.json --owner keypairs/admin.json \
        && spl-token mint -u localhost keypairs/mint_buy.json 100000000000 --recipient-owner keypairs/searcher_js.json --mint-authority keypairs/admin.json \
        && spl-token mint -u localhost keypairs/mint_sell.json 100000000000 --recipient-owner keypairs/searcher_js.json --mint-authority keypairs/admin.json \
        && spl-token mint -u localhost keypairs/mint_buy.json 100000000000 --recipient-owner keypairs/searcher_py.json --mint-authority keypairs/admin.json \
        && spl-token mint -u localhost keypairs/mint_sell.json 100000000000 --recipient-owner keypairs/searcher_py.json --mint-authority keypairs/admin.json \
        && spl-token mint -u localhost keypairs/mint_buy.json 100000000000 --recipient-owner keypairs/searcher_rust.json --mint-authority keypairs/admin.json \
        && spl-token mint -u localhost keypairs/mint_sell.json 100000000000 --recipient-owner keypairs/searcher_rust.json --mint-authority keypairs/admin.json \
        && spl-token mint -u localhost keypairs/mint_buy.json 100000000000 --recipient-owner keypairs/admin.json --mint-authority keypairs/admin.json \
        && spl-token mint -u localhost keypairs/mint_sell.json 100000000000 --recipient-owner keypairs/admin.json --mint-authority keypairs/admin.json \
        && solana airdrop 5 $(spl-token address -u localhost --owner keypairs/searcher_rust.json --token So11111111111111111111111111111111111111112 --verbose | grep 'Associated token address' | awk '{print $NF}') -u localhost \
        && spl-token sync-native keypairs/searcher_rust.json -u localhost \
        && solana airdrop 5 $(spl-token address -u localhost --owner keypairs/searcher_js.json --token So11111111111111111111111111111111111111112 --verbose | grep 'Associated token address' | awk '{print $NF}') -u localhost \
        && spl-token sync-native keypairs/searcher_js.json -u localhost \
        && solana airdrop 5 $(spl-token address -u localhost --owner keypairs/searcher_py.json --token So11111111111111111111111111111111111111112 --verbose | grep 'Associated token address' | awk '{print $NF}') -u localhost \
        && spl-token sync-native keypairs/searcher_py.json -u localhost""",
    resource_deps=["svm-setup-accounts"]
)

# setup limo global config and vaults for buy and sell tokens
RUN_CLI= "ADMIN=../../keypairs/admin.json RPC_ENV=localnet npm exec limo-cli --"
SET_GLOBAL_CONFIG = "LIMO_GLOBAL_CONFIG=$(solana-keygen pubkey ../../keypairs/limo_global_config.json)"
MINT_SELL= "$(solana-keygen pubkey %s/keypairs/mint_sell.json)" % config.main_dir
MINT_BUY= "$(solana-keygen pubkey %s/keypairs/mint_buy.json)" % config.main_dir
local_resource(
    "svm-limo-setup",
        """solana-keygen new -o ../../keypairs/limo_global_config.json -f --no-bip39-passphrase \
        && {RUN_CLI} init-global-config --global-config-file-path ../../keypairs/limo_global_config.json \
        && {SET_GLOBAL_CONFIG} {RUN_CLI} init-vault --mint {MINT_SELL} --mode execute \
        && {SET_GLOBAL_CONFIG} {RUN_CLI} init-vault --mint {MINT_BUY} --mode execute"""
        .format(RUN_CLI=RUN_CLI, SET_GLOBAL_CONFIG=SET_GLOBAL_CONFIG, MINT_SELL=MINT_SELL, MINT_BUY=MINT_BUY),
    resource_deps=["svm-create-mints"], dir="contracts/svm",
)

# create a single limo order for the searcher to bid on
local_resource(
    "svm-limo-create-order",
        "{SET_GLOBAL_CONFIG} {RUN_CLI} place-ask --quote {MINT_SELL} --base {MINT_BUY} --price 10000 --quote-amount 20"
        .format(RUN_CLI=RUN_CLI, SET_GLOBAL_CONFIG=SET_GLOBAL_CONFIG, MINT_SELL=MINT_SELL, MINT_BUY=MINT_BUY),
    resource_deps=["svm-limo-setup"], dir="contracts/svm",
)

local_resource(
    "auction-server",
    serve_cmd="source ../tilt-resources.env; source ./.env; cargo run -- run --database-url $DATABASE_URL --subwallet-private-key $RELAYER_PRIVATE_KEY --secret-key $SECRET_KEY",
    serve_dir="auction-server",
    resource_deps=["create-server-configs", "svm-build-programs", "svm-setup-accounts"],
    readiness_probe=probe(period_secs=5, http_get=http_get_action(port=9000)),
)

monitor_command = (
    "source tilt-resources.env; "
    + "poetry -C tilt-scripts run "
    + "python3 -m tilt-scripts.protocols.token_vault_monitor "
    + "--chain-id development "
    + "--rpc-url %s " % (rpc_url_anvil)
    + "--vault-contract $TOKEN_VAULT "
    + "--weth-contract $WETH "
    + "--liquidation-server-url http://localhost:9000 "
    + "--mock-pyth"
)

local_resource(
    "evm-monitor",
    serve_cmd=monitor_command,
    resource_deps=["evm-deploy-contracts", "auction-server", "create-server-configs"],
)

local_resource(
    "svm-localnet",
    serve_cmd="solana-test-validator $(./test-validator-params.sh)",
    serve_dir="contracts/svm",
    # check readiness by sending a health GET query to the RPC url
    readiness_probe=probe(
        period_secs=10,
        http_get = http_get_action(port=int(rpc_port_solana), host="localhost", scheme="http", path="/health")
    ),
    resource_deps=["svm-build-programs"],
)

local_resource(
    "svm-setup-accounts",
    "poetry -C tilt-scripts run python3 -m tilt-scripts.svm.setup_accounts --rpc-url %s" % rpc_url_solana,
    resource_deps=["svm-localnet"],
)

# need to run initialize instructions for the programs one time, script skips if already initialized
local_resource(
    "svm-initialize-programs",
    "poetry -C tilt-scripts run python3 -m tilt-scripts.svm.initialize_programs -v --file-private-key-payer keypairs/searcher_py.json --file-private-key-admin keypairs/admin.json --file-private-key-relayer-signer keypairs/relayer_signer.json --file-private-key-fee-receiver-relayer keypairs/fee_receiver_relayer.json --express-relay-program PytERJFhAKuNNuaiXkApLfWzwNwSNDACpigT3LwQfou --rpc-url %s" % rpc_url_solana,
    resource_deps=["svm-setup-accounts"]
)

# craft dummy tx, submits as a bid to auction server or submits relayer-signed tx directly to solana cluster
local_resource(
    "svm-submit-bid",
    "poetry -C tilt-scripts run python3 -m tilt-scripts.svm.dummy_tx -v --file-private-key-searcher keypairs/searcher_py.json --file-private-key-relayer-signer keypairs/relayer_signer.json --bid 100000000 --auction-server-url http://localhost:9000 --express-relay-program PytERJFhAKuNNuaiXkApLfWzwNwSNDACpigT3LwQfou --dummy-program DUmmYXYFZugRn2DS4REc5F9UbQNoxYsHP1VMZ6j5U7kZ --rpc-url %s --use-lookup-table" % rpc_url_solana,
    resource_deps=["svm-initialize-programs", "auction-server"],
)

local_resource(
    "svm-limonade",
    serve_cmd="pnpm run --prefix scripts/limonade limonade --global-config $(solana-keygen pubkey keypairs/limo_global_config.json)  --endpoint http://127.0.0.1:9000 --chain-id local-solana --api-key $(poetry -C tilt-scripts run python3 tilt-scripts/utils/create_profile.py --name limo --email limo@dourolabs.com --role protocol) --rpc-endpoint %s" % rpc_url_solana,
    resource_deps=["svm-initialize-programs", "auction-server"],
)

local_resource(
    "svm-searcher-py",
    serve_cmd="poetry run python3 -m express_relay.searcher.examples.testing_searcher_svm --endpoint-express-relay http://127.0.0.1:9000 --chain-id local-solana --api-key $(poetry -C tilt-scripts run python3 ../../tilt-scripts/utils/create_profile.py --name python_sdk --email python_sdk@dourolabs.com --role searcher) --private-key-json-file ../../keypairs/searcher_py.json --endpoint-svm http://127.0.0.1:8899 --bid 10000000 --fill-rate 4 --bid-margin 100 --with-latency",
    serve_dir="sdk/python",
    resource_deps=["svm-initialize-programs", "auction-server"],
)

js_searcher_command = (
    "JS_API_KEY=$(poetry -C tilt-scripts run python3 tilt-scripts/utils/create_profile.py --name js_sdk --email js_sdk@dourolabs.com --role searcher);"
    + "cd sdk/js;"
    + "pnpm run testing-searcher-svm --endpoint-express-relay http://127.0.0.1:9000 --chain-id local-solana --api-key $JS_API_KEY --private-key-json-file ../../keypairs/searcher_js.json --endpoint-svm http://127.0.0.1:8899 --bid 10000000 --fill-rate 4 --bid-margin 100"
)
local_resource(
    "svm-searcher-js",
    serve_cmd=js_searcher_command,
    resource_deps=["svm-initialize-programs", "auction-server"],
)

rust_searcher_command = (
    "source tilt-resources.env;"
    + "export SVM_PRIVATE_KEY_FILE=keypairs/searcher_rust.json;"
    + "cargo run -p testing-searcher -- --api-key=$(poetry -C tilt-scripts run python3 tilt-scripts/utils/create_profile.py --name rust_sdk --email rust_sdk@dourolabs.com --role searcher)"
)
local_resource(
    "rust-searcher",
    serve_cmd=rust_searcher_command,
    resource_deps=["svm-initialize-programs", "evm-deploy-contracts", "auction-server", "create-server-configs"],
)

local_resource(
    "svm-test-swap-endpoint",
    "poetry -C tilt-scripts run python3 -m tilt-scripts.svm.test_swap --file-private-key-taker keypairs/admin.json --auction-server-url http://localhost:9000 --input-mint {MINT_SELL} --output-mint {MINT_BUY} --rpc-url {RPC_URL}"
    .format(RPC_URL=rpc_url_solana, MINT_SELL=MINT_SELL, MINT_BUY=MINT_BUY),
    resource_deps=["svm-searcher-js", "rust-searcher", "svm-searcher-py"],
)
