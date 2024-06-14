load("ext://uibutton", "cmd_button", "location", "text_input")


rpc_port = "9545"
rpc_url = "http://127.0.0.1:%s" % rpc_port
ws_url = "ws://127.0.0.1:%s" % rpc_port

# Default anvil private key
private_key = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"

block_time = "2"
local_resource(
    "anvil",
    serve_cmd="anvil --gas-limit 500000000000000000 --block-time %s -p %s"
    % (block_time, rpc_port),
    readiness_probe=probe(
        period_secs=5,
        exec=exec_action(
            ["cast", "cid", "--rpc-url", rpc_url]
        ),  # get chain id as a readiness probe
    ),
)

forge_base_command = (
    "forge script script/Vault.s.sol --via-ir --private-key $PRIVATE_KEY --fork-url %s -vvv"
    % rpc_url
)

# we set automine to true before deployment and then set the interval to the block time after the deployment
# to speed up the deployment
local_resource(
    "deploy-contracts",
    "cast rp --rpc-url http://localhost:9545 evm_setAutomine true; "
    + forge_base_command
    + " --sig 'setUpLocalnet()' --broadcast; "
    + "cast rp --rpc-url http://localhost:9545 evm_setIntervalMining %s" % block_time,
    dir="per_multicall",
    env={"PRIVATE_KEY": private_key},
    resource_deps=["anvil"],
)

cmd_button(
    "vault state",
    argv=[
        "sh",
        "-c",
        "cd per_multicall; "
        + forge_base_command
        + " --sig 'getVault(uint256)' $VAULT --broadcast",
    ],
    location=location.NAV,
    resource="deploy-contracts",
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
        "cd per_multicall; "
        + forge_base_command
        + " --sig 'createLiquidatableVault()' --broadcast",
    ],
    location=location.NAV,
    resource="deploy-contracts",
    env=["PRIVATE_KEY=" + private_key],
    icon_name="add",
)

local_resource(
    "create-configs", "python3 integration.py %s %s" % (rpc_url, ws_url), resource_deps=["deploy-contracts"]
)

local_resource(
    "auction-server",
    serve_cmd="source ../tilt-resources.env; source ./.env; cargo run -- run --database-url $DATABASE_URL --subwallet-private-key $RELAYER_PRIVATE_KEY --secret-key $SECRET_KEY",
    serve_dir="auction-server",
    resource_deps=["create-configs"],
    readiness_probe=probe(period_secs=5, http_get=http_get_action(port=9000)),
)


monitor_command = (
    "source tilt-resources.env; "
    + "poetry -C per_sdk run "
    + "python3 -m per_sdk.protocols.token_vault_monitor "
    + "--chain-id development "
    + "--rpc-url %s " % (rpc_url)
    + "--vault-contract $TOKEN_VAULT "
    + "--weth-contract $WETH "
    + "--liquidation-server-url http://localhost:9000 "
    + "--mock-pyth"
)

local_resource(
    "monitor",
    serve_cmd=monitor_command,
    resource_deps=["deploy-contracts", "auction-server", "create-configs"],
)

searcher_command = (
    "source tilt-resources.env;"
    + "poetry -C per_sdk run "
    + "python3 -m per_sdk.searcher.simple_searcher "
    + "--private-key $SEARCHER_SK "
    + "--chain-id development "
    + "--verbose "
    + "--liquidation-server-url http://localhost:9000 "
    + "--opportunity-adapter-address $OPPORTUNITY_ADAPTER "
    + "--weth-address $WETH"
)
local_resource(
    "searcher",
    serve_cmd=searcher_command,
    resource_deps=["deploy-contracts", "auction-server", "create-configs"],
)
