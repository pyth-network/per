import { Router, RouterOutput } from "../types";
import { PublicKey, TransactionInstruction } from "@solana/web3.js";
import { createJupiterApiClient } from "@jup-ag/api";

const maxAccounts = 20;

export class JupiterRouter implements Router {
  private chainId: string;
  private executor: PublicKey;
  private jupiterClient = createJupiterApiClient();

  constructor(chainId: string, executor: PublicKey) {
    this.chainId = chainId;
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

    const quoteResponse = await this.jupiterClient.quoteGet({
      inputMint: tokenIn.toBase58(),
      outputMint: tokenOut.toBase58(),
      amount: Number(amountIn),
      autoSlippage: true,
      maxAutoSlippageBps: 50,
      maxAccounts: maxAccounts,
    });

    const instructions = await this.jupiterClient.swapInstructionsPost({
      swapRequest: {
        userPublicKey: this.executor.toBase58(),
        quoteResponse,
      },
    });

    const {
      computeBudgetInstructions,
      setupInstructions,
      swapInstruction,
      addressLookupTableAddresses,
    } = instructions;

    const computeBudgetInstructionsJupiter = computeBudgetInstructions.map(
      (ix: JupiterInstruction) => this.deserializeInstruction(ix)
    );
    const setupInstructionsJupiter = setupInstructions.map(
      (ix: JupiterInstruction) => this.deserializeInstruction(ix)
    );
    const ixsJupiter = [
      ...computeBudgetInstructionsJupiter,
      ...setupInstructionsJupiter,
      this.deserializeInstruction(swapInstruction),
    ];

    return {
      ixs: ixsJupiter,
      amountIn,
      amountOut: BigInt(quoteResponse.outAmount),
      lookupTableAddresses: addressLookupTableAddresses.map(
        (addr) => new PublicKey(addr)
      ),
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
