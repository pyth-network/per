import json

from solders.keypair import Keypair


def read_kp_from_json(file_path: str) -> Keypair:
    with open(file_path, "r") as f:
        sk = json.load(f)
        return Keypair.from_bytes(sk)
