import { PublicKey, TransactionInstruction } from "@solana/web3.js";

export type RouterOutput = {
  ixsComputeBudget: TransactionInstruction[];
  ixsRouter: TransactionInstruction[];
  amountIn: bigint;
  amountOut: bigint;
  lookupTableAddresses?: PublicKey[];
};

export interface Router {
  route: (
    tokenIn: PublicKey,
    tokenOut: PublicKey,
    amountIn: bigint
  ) => Promise<RouterOutput>;
}
