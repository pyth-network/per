from typing import Dict, TypedDict

from solders.pubkey import Pubkey


class SvmProgramConfig(TypedDict):
    express_relay_program: Pubkey


SVM_CONFIGS: Dict[str, SvmProgramConfig] = {
    "local-solana": {
        "express_relay_program": Pubkey.from_string(
            "PytERJFhAKuNNuaiXkApLfWzwNwSNDACpigT3LwQfou"
        ),
    },
    "development-solana": {
        "express_relay_program": Pubkey.from_string(
            "stag1NN9voD7436oFvKmy1kvRZYLLW8drKocSCt2W79"
        ),
    },
    "solana": {
        "express_relay_program": Pubkey.from_string(
            "PytERJFhAKuNNuaiXkApLfWzwNwSNDACpigT3LwQfou"
        ),
    },
}
