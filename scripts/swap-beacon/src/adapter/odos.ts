import { Adapter, ExtendedTargetCall, OdosToken, TokenToSend } from "../types";
import { Address, Hex } from "viem";
import { TokenAmount } from "@pythnetwork/express-relay-evm-js";
import axios from "axios";
import { getSwapAdapterConfig } from "../index";
import { ODOS_TOKEN_MAP } from "../const";

export class OdosAdapterError extends Error {}

export class OdosAdapter implements Adapter {
  chainIds: string[] = ["mode"];
  private baseUrl = "https://api.odos.xyz";

  private getToken(chainId: string, token: Address): OdosToken {
    const tokenMap = ODOS_TOKEN_MAP[chainId];
    if (!tokenMap) {
      throw new OdosAdapterError("Chain Id not supported");
    }
    if (!tokenMap[token.toLowerCase() as Lowercase<Address>]) {
      throw new OdosAdapterError("Token is not supported");
    }
    return tokenMap[token.toLowerCase() as Lowercase<Address>];
  }

  private async getPrice(chainId: string, token: Address): Promise<number> {
    const swapAdapterConfig = getSwapAdapterConfig(chainId);
    const response = await axios.get(
      `${this.baseUrl}/pricing/token/${swapAdapterConfig.chainIdNum}/${token}`,
    );
    return response.data.price as number;
  }

  private async estimateAmountIn(
    chainId: string,
    tokenIn: Address,
    tokenOut: Address,
    amountOut: bigint,
    threshold: number,
  ) {
    const decimalsIn = this.getToken(chainId, tokenIn).decimals;
    const decimalsOut = this.getToken(chainId, tokenOut).decimals;

    const [priceIn, priceOut] = await Promise.all([
      this.getPrice(chainId, tokenIn),
      this.getPrice(chainId, tokenOut),
    ]);

    const conversionRate =
      priceOut / 10 ** decimalsOut / (priceIn / 10 ** decimalsIn);
    return BigInt(Math.ceil(Number(amountOut) * conversionRate * threshold));
  }

  private async getPathId(
    chainId: string,
    tokenIn: Address,
    tokenOut: Address,
    amountIn?: bigint,
    amountOut?: bigint,
  ): Promise<string> {
    if (!amountIn && !amountOut) {
      throw new OdosAdapterError("amountIn or amountOut must be defined");
    }

    const swapAdapterConfig = getSwapAdapterConfig(chainId);
    // handle up to 10% slippage
    for (let threshold = 1; threshold <= 20; threshold += 1) {
      let estimatedAmountIn =
        amountIn ??
        (await this.estimateAmountIn(
          chainId,
          tokenIn,
          tokenOut,
          amountOut!,
          1 + threshold * 0.005,
        ));
      const responseQuote = await axios.post(`${this.baseUrl}/sor/quote/v2`, {
        chainId: swapAdapterConfig.chainIdNum,
        inputTokens: [
          {
            amount: estimatedAmountIn.toString(),
            tokenAddress: tokenIn,
          },
        ],
        outputTokens: [
          {
            proportion: 1,
            tokenAddress: tokenOut,
          },
        ],
        slippageLimitPercent: 0.5,
        userAddr: swapAdapterConfig.multicallAdapter,
      });

      if (amountOut) {
        if (responseQuote.data.outAmounts[0] < amountOut) {
          continue;
        }
      }

      return responseQuote.data.pathId as string;
    }

    throw new OdosAdapterError("Not enough output tokens");
  }

  async constructSwaps(
    chainId: string,
    tokenIn: Address,
    tokenOut: Address,
    amountIn?: bigint,
    amountOut?: bigint,
  ): Promise<ExtendedTargetCall[]> {
    const swapAdapterConfig = getSwapAdapterConfig(chainId);
    const pathId = await this.getPathId(
      chainId,
      tokenIn,
      tokenOut,
      amountIn,
      amountOut,
    );

    const responseTx = await axios.post(`${this.baseUrl}/sor/assemble`, {
      pathId: pathId,
      simulate: false,
      userAddr: swapAdapterConfig.multicallAdapter,
    });

    const targetCalldata: Hex = responseTx.data.transaction.data;
    const targetContract: Address = responseTx.data.transaction.to;
    const targetCallValue: bigint = BigInt(responseTx.data.transaction.value);

    const inputTokens: [{ tokenAddress: Address; amount: string }] =
      responseTx.data.inputTokens;
    const outputTokens: [{ tokenAddress: Address; amount: string }] =
      responseTx.data.outputTokens;

    const tokensToSend: TokenToSend[] = inputTokens.map(
      (inputToken: { tokenAddress: Address; amount: string }) => ({
        tokenAmount: {
          token: inputToken.tokenAddress,
          amount: BigInt(inputToken.amount),
        },
        destination: responseTx.data.transaction.to,
      }),
    );
    const tokensToReceive: TokenAmount[] = outputTokens.map(
      (outputToken: { tokenAddress: Address; amount: string }) => ({
        token: outputToken.tokenAddress as Address,
        amount: BigInt(
          amountOut
            ? outputToken.amount
            : Math.floor(Number(outputToken.amount) * 0.995).toString(),
        ),
      }),
    );

    return [
      {
        targetContract,
        targetCalldata,
        targetCallValue,
        tokensToSend,
        tokensToReceive,
      },
    ];
  }
}
