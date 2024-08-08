import { OdosAdapter } from "./adapter/odos";
import { SWAP_ADAPTER_CONFIGS } from "./const";
import {
  Client,
  Opportunity,
  ChainId,
  TokenAmount,
} from "@pythnetwork/express-relay-evm-js";
import { Adapter, ExtendedTargetCall, TargetCall } from "./types";
import { Address, Hex, encodeFunctionData } from "viem";
import { multicallAbi } from "./abi";
import yargs from "yargs";
import { hideBin } from "yargs/helpers";

export class SwapBeaconError extends Error {}

export function getSwapAdapterConfig(chainId: string) {
  const swapAdapterConfig = SWAP_ADAPTER_CONFIGS[chainId];
  if (!swapAdapterConfig) {
    throw new SwapBeaconError(
      `Opportunity adapter config not found for chain id: ${chainId}`
    );
  }
  return swapAdapterConfig;
}

export class SwapBeacon {
  private client: Client;
  private adapters: Adapter[];
  private chainIds: string[];

  constructor(endpoint: string, _chainIds: string[]) {
    this.client = new Client(
      {
        baseUrl: endpoint,
      },
      undefined,
      this.opportunityHandler.bind(this)
    );
    this.chainIds = _chainIds;
    this.adapters = [new OdosAdapter()];
  }

  private async getOptimalAdapter(
    chainId: ChainId,
    tokenIn: Address,
    tokenOut: Address,
    amountIn?: bigint
  ): Promise<Adapter | undefined> {
    if (tokenIn.toLocaleLowerCase() === tokenOut.toLocaleLowerCase()) {
      return undefined;
    }
    return this.adapters[0];
    // const prices = await Promise.all(
    //   this.adapters.map((adapter) =>
    //     adapter.getPrice(chainId, tokenIn, tokenOut, amountIn)
    //   )
    // );

    // return this.adapters[
    //   prices.reduce(
    //     (prev, curr, currIndex) => (prices[prev] < curr ? currIndex : prev),
    //     0
    //   )
    // ];
  }

  private makeMulticallCalldata(
    opportunity: Opportunity,
    swapsSell: ExtendedTargetCall[],
    swapsBuy: ExtendedTargetCall[],
    sellTokens: TokenAmount[],
    buyTokens: TokenAmount[]
  ): Hex {
    const originalTargetCall = {
      targetContract: opportunity.targetContract,
      targetCalldata: opportunity.targetCalldata,
      targetCallValue: opportunity.targetCallValue,
      tokensToSend: opportunity.sellTokens.map((tokenAmount) => ({
        tokenAmount: tokenAmount,
        destination: opportunity.targetContract,
      })),
    };
    const swapsSellTargetCalls = swapsSell.map((swap) => ({
      targetContract: swap.targetContract,
      targetCalldata: swap.targetCalldata,
      targetCallValue: swap.targetCallValue,
      tokensToSend: swap.tokensToSend,
    }));
    const swapsBuyTargetCalls = swapsBuy.map((swap) => ({
      targetContract: swap.targetContract,
      targetCalldata: swap.targetCalldata,
      targetCallValue: swap.targetCallValue,
      tokensToSend: swap.tokensToSend,
    }));
    const multicallTargetCalls = [
      ...swapsSellTargetCalls,
      originalTargetCall,
      ...swapsBuyTargetCalls,
    ];

    return encodeFunctionData({
      abi: [multicallAbi],
      args: [[sellTokens, buyTokens, multicallTargetCalls]],
    });
  }

  private createSwapOpportunity(
    opportunity: Opportunity,
    base: Address,
    swapsSell: ExtendedTargetCall[],
    swapsBuy: ExtendedTargetCall[]
  ): Opportunity | undefined {
    const targetContract =
      SWAP_ADAPTER_CONFIGS[opportunity.chainId].multicallAdapter;
    const targetCallValue = BigInt(
      BigInt(
        swapsSell.reduce(
          (prev, curr) => BigInt(prev + curr.targetCallValue),
          0n
        ) +
          swapsBuy.reduce(
            (prev, curr) => BigInt(prev + curr.targetCallValue),
            0n
          )
      ) + opportunity.targetCallValue
    );

    // If base is equal to opportunity target token, then we don't need to swap
    let sellToken = opportunity.sellTokens
      .filter(({ token }) => token.toLowerCase() === base.toLowerCase())
      .reduce((acc, val) => BigInt(acc + val.amount), 0n);
    sellToken += swapsSell.reduce(
      (acc, val) =>
        BigInt(
          acc +
            val.tokensToSend.reduce(
              (acc, val) => BigInt(acc + val.tokenAmount.amount),
              0n
            )
        ),
      0n
    );

    let buyToken = opportunity.buyTokens
      .filter(({ token }) => token.toLowerCase() === base.toLowerCase())
      .reduce((acc, val) => BigInt(acc + val.amount), 0n);
    buyToken += swapsBuy.reduce(
      (acc, val) =>
        BigInt(
          acc +
            val.tokensToReceive.reduce(
              (acc, val) => BigInt(acc + val.amount),
              0n
            )
        ),
      0n
    );

    if (buyToken <= sellToken) {
      console.log(
        "Sell token is greater than buy token for opportunity:",
        opportunity
      );
      console.log("Sell token:", sellToken.toString());
      console.log("Buy token:", buyToken.toString());
      console.log("Swaps sell:", JSON.stringify(swapsSell));
      console.log("Swaps buy:", JSON.stringify(swapsBuy));
      return undefined;
    }

    const sellTokens = [
      {
        token: base,
        amount: sellToken,
      },
    ];
    const buyTokens = [
      {
        token: base,
        amount: buyToken,
      },
    ];

    const targetCalldata = this.makeMulticallCalldata(
      opportunity,
      swapsSell,
      swapsBuy,
      sellTokens,
      buyTokens
    );

    return {
      ...opportunity,
      targetContract,
      targetCalldata,
      targetCallValue,
      sellTokens,
      buyTokens,
    };
  }

  async convertOpportunity(
    opportunity: Opportunity,
    base: Address
  ): Promise<Opportunity | undefined> {
    const promisesOptimalAdaptersSell = opportunity.sellTokens.map(
      (sellToken) =>
        this.getOptimalAdapter(opportunity.chainId, base, sellToken.token)
    );
    const promisesOptimalAdaptersBuy = opportunity.buyTokens.map((buyToken) =>
      this.getOptimalAdapter(
        opportunity.chainId,
        buyToken.token,
        base,
        buyToken.amount
      )
    );

    const [optimalAdaptersSell, optimalAdaptersBuy] = await Promise.all([
      Promise.all(promisesOptimalAdaptersSell),
      Promise.all(promisesOptimalAdaptersBuy),
    ]);

    const swapsSell = (
      await Promise.all(
        optimalAdaptersSell.map(async (adapter, index) => {
          if (adapter === undefined) {
            return [];
          }
          return adapter.constructSwaps(
            opportunity.chainId,
            base,
            opportunity.sellTokens[index].token,
            undefined,
            opportunity.sellTokens[index].amount
          );
        })
      )
    ).reduce((acc, val) => acc.concat(val), []);
    const swapsBuy = (
      await Promise.all(
        optimalAdaptersBuy.map(async (adapter, index) => {
          if (adapter === undefined) {
            return [];
          }
          return adapter.constructSwaps(
            opportunity.chainId,
            opportunity.buyTokens[index].token,
            base,
            opportunity.buyTokens[index].amount,
            undefined
          );
        })
      )
    ).reduce((acc, val) => acc.concat(val), []);

    return this.createSwapOpportunity(opportunity, base, swapsSell, swapsBuy);
  }

  async opportunityHandler(opportunity: Opportunity) {
    const swapAdapterConfig = getSwapAdapterConfig(opportunity.chainId);

    if (
      opportunity.targetContract.toLowerCase() ===
      swapAdapterConfig.multicallAdapter.toLowerCase()
    ) {
      return;
    }

    await Promise.all(
      swapAdapterConfig.liquidAssets.map(async (base) => {
        const convertedOpportunity = await this.convertOpportunity(
          opportunity,
          base
        );
        if (!convertedOpportunity) {
          return;
        }

        const { opportunityId, ...params } = convertedOpportunity;
        try {
          await this.client.submitOpportunity(params);
        } catch (error) {
          console.error(
            `Failed to submit opportunity ${opportunityId}: ${error}`
          );
        }
      })
    );
  }

  async start() {
    try {
      await this.client.subscribeChains(this.chainIds);
      console.log(
        `Subscribed to chain ${this.chainIds}. Waiting for opportunities...`
      );
    } catch (error) {
      console.error(error);
      this.client.websocket?.close();
    }
  }
}

const argv = yargs(hideBin(process.argv))
  .option("endpoint", {
    description:
      "Express relay endpoint. e.g: https://per-staging.dourolabs.app/",
    type: "string",
    default: "https://pyth-express-relay-mainnet.asymmetric.re/",
  })
  .option("chain-ids", {
    description:
      "Chain ids seperated by comma to listen and convert opportunities for.",
    type: "string",
    default: "mode",
  })
  .help()
  .alias("help", "h")
  .parseSync();

async function run() {
  const beacon = new SwapBeacon(argv.endpoint, argv["chain-ids"].split(","));
  await beacon.start();
}

// eslint-disable-next-line @typescript-eslint/ban-ts-comment
// @ts-ignore: Unreachable code error
BigInt.prototype.toJSON = function (): string {
  return this.toString();
};

run();
