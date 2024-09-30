import { Router, RouterOutput } from "../types";
import { PublicKey } from "@solana/web3.js";

const jupiterBaseUrl = "https://quote-api.jup.ag/v6/";
const jupiterQuoteUrl = new URL("quote", jupiterBaseUrl);
const jupiterSwapIxsUrl = new URL("swap-instructions", jupiterBaseUrl);
const maxAccounts = 64;

export class JupiterRouter implements Router {
  async route(
    chainId: string,
    tokenIn: PublicKey,
    tokenOut: PublicKey,
    amountIn: bigint,
    executor: PublicKey
  ): Promise<RouterOutput> {
    if (chainId !== "solana") {
      throw new Error("Chain Id not supported");
    }

    const quoteResponse = await (
      await fetch(
        `${jupiterQuoteUrl.toString()}?inputMint=${tokenIn.toBase58()}&outputMint=${tokenOut.toBase58()}&amount=${amountIn}&autoSlippage=true&maxAutoSlippageBps=50&maxAccounts=${maxAccounts}`
      )
    ).json();

    const instructions = await (
      await fetch(jupiterSwapIxsUrl.toString(), {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          quoteResponse,
          userPublicKey: executor.toBase58(),
        }),
      })
    ).json();

    return {
      ixs: instructions,
      amountIn,
      amountOut: BigInt(quoteResponse.outAmount),
    };
  }
}
