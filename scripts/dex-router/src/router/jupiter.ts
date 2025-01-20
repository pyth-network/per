import { Router, RouterOutput } from "../types";
import { PublicKey, TransactionInstruction } from "@solana/web3.js";
import {
  createJupiterApiClient,
  DefaultApi,
  Instruction as JupiterInstruction,
} from "@jup-ag/api";

const MAX_SLIPPAGE_BPS = 50;

export class JupiterRouter implements Router {
  private executor: PublicKey;
  private maxAccounts: number;
  private jupiterClient: DefaultApi;

  constructor(
    executor: PublicKey,
    maxAccounts: number,
    basePath: string,
    apiKey?: string,
  ) {
    this.executor = executor;
    this.maxAccounts = maxAccounts;
    if (apiKey) {
      this.jupiterClient = createJupiterApiClient({
        basePath,
        apiKey: apiKey,
      });
    } else {
      this.jupiterClient = createJupiterApiClient({
        basePath,
      });
    }
  }

  async route(
    tokenIn: PublicKey,
    tokenOut: PublicKey,
    amountIn: bigint,
  ): Promise<RouterOutput> {
    const quoteResponse = await this.jupiterClient.quoteGet({
      inputMint: tokenIn.toBase58(),
      outputMint: tokenOut.toBase58(),
      amount: Number(amountIn),
      autoSlippage: true,
      maxAutoSlippageBps: MAX_SLIPPAGE_BPS,
      maxAccounts: this.maxAccounts,
    });

    const instructions = await this.jupiterClient.swapInstructionsPost({
      swapRequest: {
        userPublicKey: this.executor.toBase58(),
        quoteResponse,
      },
    });

    const { setupInstructions, swapInstruction, addressLookupTableAddresses } =
      instructions;

    const ixsSetupJupiter = setupInstructions.map((ix) =>
      this.convertInstruction(ix),
    );
    const ixsJupiter = [
      ...ixsSetupJupiter,
      this.convertInstruction(swapInstruction),
    ];

    return {
      ixsRouter: ixsJupiter,
      amountIn,
      amountOut: BigInt(quoteResponse.outAmount),
      lookupTableAddresses: addressLookupTableAddresses.map(
        (addr) => new PublicKey(addr),
      ),
    };
  }

  private convertInstruction(
    instruction: JupiterInstruction,
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
