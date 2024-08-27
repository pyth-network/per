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
    dir="contracts",
    env={"PRIVATE_KEY": private_key},
    resource_deps=["evm-anvil"],
)

cmd_button(
    "vault state",
    argv=[
        "sh",
        "-c",
        "cd contracts; "
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
        "cd contracts; "
        + forge_base_command
        + " --sig 'createLiquidatableVault()' --broadcast",
    ],
    location=location.NAV,
    resource="evm-deploy-contracts",
    env=["PRIVATE_KEY=" + private_key],
    icon_name="add",
)

local_resource(
    "evm-create-configs", "python3 evm_integration.py %s %s" % (rpc_url_anvil, ws_url_anvil), resource_deps=["evm-deploy-contracts"]
)

local_resource(
    "auction-server",
    serve_cmd="source ../tilt-resources.env; source ./.env; cargo run -- run --database-url $DATABASE_URL --subwallet-private-key $RELAYER_PRIVATE_KEY --secret-key $SECRET_KEY",
    serve_dir="auction-server",
    resource_deps=["evm-create-configs"],
    readiness_probe=probe(period_secs=5, http_get=http_get_action(port=9000)),
)

monitor_command = (
    "source tilt-resources.env; "
    + "poetry -C per_sdk run "
    + "python3 -m per_sdk.protocols.token_vault_monitor "
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
    resource_deps=["evm-deploy-contracts", "auction-server", "evm-create-configs"],
)

evm_searcher_command = (
    "source tilt-resources.env;"
    + "poetry -C per_sdk run "
    + "python3 -m per_sdk.searcher.simple_searcher "
    + "--private-key $SEARCHER_SK "
    + "--chain-id development "
    + "--chain-id-num $CHAIN_ID_NUM "
    + "--verbose "
    + "--liquidation-server-url http://localhost:9000 "
    + "--adapter-factory-address $ADAPTER_FACTORY "
    + "--adapter-init-bytecode-hash $ADAPTER_BYTECODE_HASH "
    + "--weth-address $WETH "
    + "--permit2-address $PERMIT2 "
)
local_resource(
    "evm-searcher",
    serve_cmd=evm_searcher_command,
    resource_deps=["evm-deploy-contracts", "auction-server", "evm-create-configs"],
)


# svm resources
local_resource(
    "svm-build-programs",
    "cargo build-sbf",
    dir="express_relay",
)

local_resource(
    "svm-localnet",
    serve_cmd="solana-test-validator $(./test-validator-params.sh)",
    serve_dir="express_relay",
    # check readiness by sending a health GET query to the RPC url
    readiness_probe=probe(
        period_secs=10,
        http_get = http_get_action(port=int(rpc_port_solana), host="localhost", scheme="http", path="/health")
    ),
    resource_deps=["svm-build-programs"],
)

local_resource(
    "svm-setup-accounts",
    "poetry -C per_sdk run python3 -m per_sdk.svm.setup_accounts --rpc-url %s" % rpc_url_solana,
    resource_deps=["svm-localnet"]
)

# need to run initialize instructions for the programs one time, script skips if already initialized
local_resource(
    "svm-initialize-programs",
    "poetry -C per_sdk run python3 -m per_sdk.svm.initialize_programs -v --file-private-key-payer keypairs/searcher.json --file-private-key-admin keypairs/admin.json --file-private-key-relayer-signer keypairs/relayer_signer.json --express-relay-program GwEtasTAxdS9neVE4GPUpcwR7DB7AizntQSPcG36ubZM --dummy-program HYCgALnu6CM2gkQVopa1HGaNf8Vzbs9bomWRiKP267P3 --rpc-url %s" % rpc_url_solana,
    resource_deps=["svm-setup-accounts"]
)

# craft dummy tx, submits as a bid to auction server or submits relayer-signed tx directly to solana cluster
local_resource(
    "svm-submit-bid",
    "poetry -C per_sdk run python3 -m per_sdk.svm.dummy_tx -v --file-private-key-searcher keypairs/searcher.json --file-private-key-relayer-signer keypairs/relayer_signer.json --bid 100 --auction-server-url http://localhost:9000 --express-relay-program GwEtasTAxdS9neVE4GPUpcwR7DB7AizntQSPcG36ubZM --dummy-program HYCgALnu6CM2gkQVopa1HGaNf8Vzbs9bomWRiKP267P3",
    resource_deps=["svm-initialize-programs", "auction-server"],
)
