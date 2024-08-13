import { Address, Hex } from "viem";
import { TokenAmount } from "@pythnetwork/express-relay-evm-js";

export type SwapAdapterConfig = {
  /**
   * The chain id as a string
   */
  chainId: string;
  /**
   * The chain id as a number
   */
  chainIdNum: number;
  /**
   * The multicall adapter address
   */
  multicallAdapter: Address;
  /**
   * List of liquid assets to swap into/from
   */
  liquidAssets: Address[];
};

export type TokenToSend = {
  tokenAmount: TokenAmount;
  destination: Address;
};

export type TargetCall = {
  targetContract: Address;
  targetCalldata: Hex;
  targetCallValue: bigint;
  tokensToSend: TokenToSend[];
};

export type ExtendedTargetCall = TargetCall & {
  tokensToReceive: TokenAmount[];
};

export interface Adapter {
  chainIds: string[];
  constructSwaps: (
    chainId: string,
    tokenIn: Address,
    tokenOut: Address,
    amountIn?: bigint,
    amountOut?: bigint
  ) => Promise<ExtendedTargetCall[]>;
}

export type OdosToken = {
  name: string;
  symbol: string;
  decimals: number;
  assetId: string;
  assetType: string;
  protocolId: string | null;
  isRebasing: boolean;
};
