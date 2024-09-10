import json
import logging
from pathlib import Path

from solders.keypair import Keypair


def read_kp_from_json(file_path: Path) -> Keypair:
    with open(file_path, "r") as f:
        sk = json.load(f)
        return Keypair.from_bytes(sk)


def configure_logger(logger: logging.Logger, verbose: bool = False):
    logger.setLevel(logging.DEBUG if verbose else logging.INFO)
    log_handler = logging.StreamHandler()
    formatter = logging.Formatter(
        "%(asctime)s %(levelname)s:%(name)s:%(module)s %(message)s",
        datefmt="%Y-%m-%d %H:%M:%S",
    )
    log_handler.setFormatter(formatter)
    logger.addHandler(log_handler)
