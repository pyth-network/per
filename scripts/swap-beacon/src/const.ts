import { SwapAdapterConfig } from "./types";

export const SWAP_ADAPTER_CONFIGS: Record<string, SwapAdapterConfig> = {
  mode: {
    chainId: "mode",
    chainIdNum: 34443,
    multicallAdapter: "0x9fcDCAb0A147e799Fa866594B2c4c20F4eF29F37",
    liquidAssets:
      // ["0x80137510979822322193FC997d400D5A6C747bf7"]
      ["0xd988097fb8612cc24eeC14542bC03424c656005f"],
    //, "0x4200000000000000000000000000000000000006"], TODO: uncomment
  },
};
