import { OdosAdapter } from "./adapter/odos";
import { SWAP_ADAPTER_CONFIGS } from "./const";
import {
  Client,
  Opportunity,
  TokenAmount,
} from "@pythnetwork/express-relay-evm-js";
import { Adapter, ExtendedTargetCall, TargetCall } from "./types";
import { Address, Hex, encodeFunctionData, isAddressEqual } from "viem";
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
  private pingTimeout: NodeJS.Timeout | undefined;
  private readonly adapters: Adapter[];
  private readonly chainIds: string[];
  private readonly PING_INTERVAL = 30000;

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

  private getOptimalAdapter(
    tokenIn: Address,
    tokenOut: Address
  ): Adapter | undefined {
    if (isAddressEqual(tokenIn, tokenOut)) {
      return undefined;
    }
    return this.adapters[0];
  }

  private makeMulticallCalldata(
    opportunity: Opportunity,
    swapsSell: ExtendedTargetCall[],
    swapsBuy: ExtendedTargetCall[],
    sellTokens: TokenAmount[],
    buyTokens: TokenAmount[]
  ): Hex {
    const originalTargetCall = {
      ...opportunity,
      tokensToSend: opportunity.sellTokens.map((tokenAmount) => ({
        tokenAmount: tokenAmount,
        destination: opportunity.targetContract,
      })),
    };
    const targetCalls = [...swapsSell, originalTargetCall, ...swapsBuy];

    return encodeFunctionData({
      abi: [multicallAbi],
      args: [
        {
          sellTokens,
          buyTokens,
          targetCalls,
        },
      ],
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
    const targetCallValue =
      swapsSell.reduce((prev, curr) => prev + curr.targetCallValue, 0n) +
      swapsBuy.reduce((prev, curr) => prev + curr.targetCallValue, 0n) +
      opportunity.targetCallValue;

    // If base is equal to opportunity target token, then we don't need to swap
    let sellToken = opportunity.sellTokens
      .filter(({ token }) => isAddressEqual(token, base))
      .reduce((acc, val) => acc + val.amount, 0n);
    sellToken += swapsSell.reduce(
      (acc, val) =>
        acc +
        val.tokensToSend.reduce((acc, val) => acc + val.tokenAmount.amount, 0n),
      0n
    );

    let buyToken = opportunity.buyTokens
      .filter(({ token }) => isAddressEqual(token, base))
      .reduce((acc, val) => acc + val.amount, 0n);
    buyToken += swapsBuy.reduce(
      (acc, val) =>
        acc +
        val.tokensToReceive.reduce((acc, val) => BigInt(acc + val.amount), 0n),
      0n
    );

    if (buyToken <= sellToken) {
      console.log(
        "Sell token is greater than buy token for opportunity:",
        opportunity
      );
      console.log("Sell token and swap:", sellToken.toString());
      console.dir(swapsSell, { depth: null });
      console.log("Buy token and swap:", buyToken.toString());
      console.dir(swapsBuy, { depth: null });
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

  private async getSwapsSell(
    opportunity: Opportunity,
    base: Address
  ): Promise<ExtendedTargetCall[]> {
    return (
      await Promise.all(
        opportunity.sellTokens.map(async ({ token, amount }) => {
          const adapter = this.getOptimalAdapter(base, token);
          if (!adapter) {
            return [];
          }
          return adapter.constructSwaps(
            opportunity.chainId,
            base,
            token,
            undefined,
            amount
          );
        })
      )
    ).reduce((acc, val) => acc.concat(val), []);
  }

  private async getSwapsBuy(
    opportunity: Opportunity,
    base: Address
  ): Promise<ExtendedTargetCall[]> {
    return (
      await Promise.all(
        opportunity.buyTokens.map(async ({ token, amount }) => {
          const adapter = this.getOptimalAdapter(token, base);
          if (!adapter) {
            return [];
          }
          return adapter.constructSwaps(
            opportunity.chainId,
            token,
            base,
            amount,
            undefined
          );
        })
      )
    ).reduce((acc, val) => acc.concat(val), []);
  }

  async convertOpportunity(
    opportunity: Opportunity,
    base: Address
  ): Promise<Opportunity | undefined> {
    const [swapsSell, swapsBuy] = await Promise.all([
      this.getSwapsSell(opportunity, base),
      this.getSwapsBuy(opportunity, base),
    ]);
    return this.createSwapOpportunity(opportunity, base, swapsSell, swapsBuy);
  }

  async opportunityHandler(opportunity: Opportunity) {
    console.log("Received opportunity:", opportunity.opportunityId);
    const swapAdapterConfig = getSwapAdapterConfig(opportunity.chainId);

    if (
      isAddressEqual(
        opportunity.targetContract,
        swapAdapterConfig.multicallAdapter
      )
    ) {
      return;
    }

    await Promise.all(
      swapAdapterConfig.liquidAssets.map(async (base) => {
        try {
          const convertedOpportunity = await this.convertOpportunity(
            opportunity,
            base
          );
          if (!convertedOpportunity) {
            return;
          }
          await this.client.submitOpportunity(convertedOpportunity);
          console.log(
            "Submitted converted opportunity. Original id:",
            opportunity.opportunityId
          );
        } catch (error) {
          console.error(
            `Failed to convert and submit opportunity ${JSON.stringify(
              opportunity
            )}: ${error}`
          );
        }
      })
    );
  }

  heartbeat() {
    if (this.pingTimeout !== undefined) clearTimeout(this.pingTimeout);

    this.pingTimeout = setTimeout(() => {
      console.error("Received no ping. Terminating connection.");
      this.client.websocket.terminate();
    }, this.PING_INTERVAL + 2000); // 2 seconds for latency
  }

  async start() {
    try {
      await this.client.subscribeChains(this.chainIds);
      console.log(
        `Subscribed to chain ${this.chainIds}. Waiting for opportunities...`
      );
      this.heartbeat();
      this.client.websocket.on("ping", () => {
        this.heartbeat();
      });
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
    description: "Chain ids to listen and convert opportunities for.",
    type: "array",
    default: ["mode"],
  })
  .help()
  .alias("help", "h")
  .parseSync();

async function run() {
  const beacon = new SwapBeacon(argv.endpoint, argv["chain-ids"].map(String));
  await beacon.start();
}

// eslint-disable-next-line @typescript-eslint/ban-ts-comment
// @ts-ignore: Unreachable code error
BigInt.prototype.toJSON = function (): string {
  return this.toString();
};

run();
