import { Router, RouterOutput } from "../types";
import { Connection, PublicKey, TransactionInstruction } from "@solana/web3.js";

const jupiterBaseUrl = "https://quote-api.jup.ag/v6/";
const jupiterQuoteUrl = new URL("quote", jupiterBaseUrl);
const jupiterSwapIxsUrl = new URL("swap-instructions", jupiterBaseUrl);
const maxAccounts = 20;

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
    if (!["mainnet-beta-solana", "development-solana"].includes(this.chainId)) {
      throw new Error("Jupiter error: chain id not supported");
    }

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
