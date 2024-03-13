load('ext://configmap', 'configmap_create')
default_registry("192824654885.dkr.ecr.eu-west-2.amazonaws.com", single_name="development")
allow_k8s_contexts(["arn:aws:eks:eu-west-2:192824654885:cluster/violet"])
if k8s_namespace() == 'default':
  fail("failing early to avoid deploying to 'default' namespace")

docker_build("server", ".",
             only=["./auction-server", "./vault-simulator", "./per_multicall"],
             ignore=["./auction-server/target", "./auction-server/config.yaml", "./vault-simulator/target", "./per_multicall/lib", "./per_multicall/node_modules"],
             dockerfile="./Dockerfile")
k8s_yaml("./tilt/deployment.yaml")
configmap_create('auction-server-config', from_file=['config.yaml=./tilt/config.yaml'])
k8s_resource("per-server", port_forwards=["9000:9000"])
