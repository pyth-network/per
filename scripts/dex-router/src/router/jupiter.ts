import { Router, RouterOutput } from "../types";
import { Connection, PublicKey, TransactionInstruction } from "@solana/web3.js";

const jupiterBaseUrl = "https://quote-api.jup.ag/v6/";
const jupiterQuoteUrl = new URL("quote", jupiterBaseUrl);
const jupiterSwapIxsUrl = new URL("swap-instructions", jupiterBaseUrl);
const maxAccounts = 30;

export class JupiterRouter implements Router {
  private chainId: string;
  private connection: Connection;
  private executor: PublicKey;

  constructor(chainId: string, connection: Connection, executor: PublicKey) {
    this.chainId = chainId;
    this.connection = connection;
    this.executor = executor;
  }

  async route(
    tokenIn: PublicKey,
    tokenOut: PublicKey,
    amountIn: bigint
  ): Promise<RouterOutput> {
    if (!["solana", "development-solana"].includes(this.chainId)) {
      throw new Error("Jupiter error: chain id not supported");
    }

    // // TODO: REMOVE THIS, ONLY FOR TESTING W FAKE TOKENS
    // if (
    //   tokenIn.equals(
    //     new PublicKey("USDCHDcjejXG1tqnrX4SfvsB2TGp8xGgTHXqxcoSeF2")
    //   )
    // ) {
    //   tokenIn = new PublicKey("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
    // }
    // if (
    //   tokenOut.equals(
    //     new PublicKey("USDCHDcjejXG1tqnrX4SfvsB2TGp8xGgTHXqxcoSeF2")
    //   )
    // ) {
    //   tokenOut = new PublicKey("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
    // }
    // if (
    //   tokenIn.equals(
    //     new PublicKey("WSoLZvwZh5mQDEpHUsvxwbnM1yGbW3Qd9rEya7GJCPX")
    //   )
    // ) {
    //   tokenIn = new PublicKey("So11111111111111111111111111111111111111112");
    // }
    // if (
    //   tokenOut.equals(
    //     new PublicKey("WSoLZvwZh5mQDEpHUsvxwbnM1yGbW3Qd9rEya7GJCPX")
    //   )
    // ) {
    //   tokenOut = new PublicKey("So11111111111111111111111111111111111111112");
    // }

    const quoteResponse = await (
      await fetch(
        `${jupiterQuoteUrl.toString()}?inputMint=${tokenIn.toBase58()}&outputMint=${tokenOut.toBase58()}&amount=${amountIn}&autoSlippage=true&maxAutoSlippageBps=50&maxAccounts=${maxAccounts}`
      )
    ).json();
    if (quoteResponse.error !== undefined) {
      throw new Error(`Jupiter error: ${quoteResponse.error}`);
    }

    const instructions = await (
      await fetch(jupiterSwapIxsUrl.toString(), {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          quoteResponse,
          userPublicKey: this.executor.toBase58(),
          asLegacyTransaction: false,
        }),
      })
    ).json();
    if (instructions.error !== undefined) {
      throw new Error(`Jupiter error: ${instructions.error}`);
    }

    const {
      tokenLedgerInstruction,
      computeBudgetInstructions,
      setupInstructions,
      swapInstruction,
      cleanupInstruction,
      addressLookupTableAddresses,
    } = instructions;

    const setupInstructionsJupiter: TransactionInstruction[] =
      setupInstructions.map((ix: JupiterInstruction) =>
        this.deserializeInstruction(ix)
      );
    const ixsJupiter = [
      ...setupInstructionsJupiter,
      this.deserializeInstruction(swapInstruction),
    ];

    return {
      ixs: ixsJupiter,
      amountIn,
      amountOut: BigInt(quoteResponse.outAmount),
      lookupTableAddresses: addressLookupTableAddresses,
    };
  }

  private deserializeInstruction(
    instruction: JupiterInstruction
  ): TransactionInstruction {
    return new TransactionInstruction({
      programId: new PublicKey(instruction.programId),
      keys: instruction.accounts.map((key) => ({
        pubkey: new PublicKey(key.pubkey),
        isSigner: key.isSigner,
        isWritable: key.isWritable,
      })),
      data: Buffer.from(instruction.data, "base64"),
    });
  }
}

export type JupiterInstruction = {
  programId: string;
  accounts: {
    pubkey: string;
    isSigner: boolean;
    isWritable: boolean;
  }[];
  data: string;
};
