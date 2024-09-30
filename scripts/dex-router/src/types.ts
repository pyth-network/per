import { PublicKey, TransactionInstruction } from "@solana/web3.js";

export type RouterOutput = {
  ixs: TransactionInstruction[];
  amountIn: bigint;
  amountOut: bigint;
};

export interface Router {
  route: (
    chainId: string,
    tokenIn: PublicKey,
    tokenOut: PublicKey,
    amountIn: bigint,
    executor: PublicKey
  ) => Promise<RouterOutput>;
}
