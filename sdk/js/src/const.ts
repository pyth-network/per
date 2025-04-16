import { PublicKey } from "@solana/web3.js";
import { SvmConstantsConfig } from "./types";

export const SVM_CONSTANTS: Record<string, SvmConstantsConfig> = {
  "local-solana": {
    expressRelayProgram: new PublicKey(
      "PytERJFhAKuNNuaiXkApLfWzwNwSNDACpigT3LwQfou",
    ),
  },
  "development-solana": {
    expressRelayProgram: new PublicKey(
      "stag1NN9voD7436oFvKmy1kvRZYLLW8drKocSCt2W79",
    ),
  },
  solana: {
    expressRelayProgram: new PublicKey(
      "PytERJFhAKuNNuaiXkApLfWzwNwSNDACpigT3LwQfou",
    ),
  },
};
