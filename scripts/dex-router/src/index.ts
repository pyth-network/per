import { Client, Opportunity } from "@pythnetwork/express-relay-js";
import { SVM_RPC_ENDPOINTS } from "./const";
import { Router, RouterOutput } from "./types";
import { JupiterRouter } from "./router/jupiter";
import yargs from "yargs";
import { hideBin } from "yargs/helpers";
import { Keypair } from "@solana/web3.js";
import * as anchor from "@coral-xyz/anchor";

export class DexRouter {
  private client: Client;
  private pingTimeout: NodeJS.Timeout | undefined;
  private readonly routers: Router[];
  private readonly executor: Keypair;
  private readonly chainIds: string[];
  private readonly PING_INTERVAL = 30000;

  constructor(endpoint: string, skExecutor: string, chainIds: string[]) {
    this.client = new Client(
      {
        baseUrl: endpoint,
      },
      undefined,
      this.opportunityHandler.bind(this)
    );
    this.routers = [new JupiterRouter()];
    this.executor = Keypair.fromSecretKey(
      anchor.utils.bytes.bs58.decode(skExecutor)
    );
    this.chainIds = chainIds;
  }

  async opportunityHandler(opportunity: Opportunity) {
    console.log("Received opportunity:", opportunity.opportunityId);

    if (!(opportunity.chainId in SVM_RPC_ENDPOINTS)) {
      return;
    }
    const rpcEndpoint = SVM_RPC_ENDPOINTS[opportunity.chainId];

    // TODO: grab opportunity details from chain?
    // TODO: route opportunity to appropriate routers
    let routerOutputs: RouterOutput[] = [];
    // for (const router of this.routers) {
    //     const routerOutput = await router.route(
    //         opportunity.chainId,
    //         opportunity.tokenIn,
    //         opportunity.tokenOut,
    //         opportunity.amountIn,
    //         this.executor
    //     );
    //     routerOutputs.push(routerOutput);
    // }

    // TODO: pick best router based on output
    const routerBest = routerOutputs.reduce((prev, curr) => {
      return prev.amountOut > curr.amountOut ? prev : curr;
    });

    // TODO: submit bid
    let routerIxs = routerBest.ixs;
    // await this.client.submitBid(bid, true);

    throw new Error("Method not implemented.");
  }

  heartbeat() {
    if (this.pingTimeout !== undefined) clearTimeout(this.pingTimeout);

    this.pingTimeout = setTimeout(() => {
      console.error("Received no ping. Terminating connection.");
      this.client.websocket?.terminate();
    }, this.PING_INTERVAL + 2000); // 2 seconds for latency
  }

  async start() {
    try {
      await this.client.subscribeChains(this.chainIds);
      console.log(
        `Subscribed to chains ${this.chainIds}. Waiting for opportunities...`
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
  .option("sk-executor", {
    description:
      "Secret key of executor to submit routed transactions with. In 64-byte base58 format",
    type: "string",
    demandOption: true,
  })
  .option("chain-ids", {
    description: "Chain ids to listen and submit routed bids for.",
    type: "array",
    default: ["solana"],
  })
  .help()
  .alias("help", "h")
  .parseSync();

async function run() {
  const dexRouter = new DexRouter(
    argv.endpoint,
    argv["sk-executor"],
    argv["chain-ids"].map(String)
  );
  await dexRouter.start();
}

run();
