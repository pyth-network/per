import {
  BidStatusUpdate,
  Client,
  ExpressRelaySvmConfig,
  Opportunity,
  OpportunitySvm,
  SVM_CONSTANTS,
} from "@pythnetwork/express-relay-js";
import { Router, RouterOutput } from "./types";
import { JupiterRouter } from "./router/jupiter";
import yargs from "yargs";
import { hideBin } from "yargs/helpers";
import {
  AddressLookupTableAccount,
  Connection,
  Keypair,
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import * as anchor from "@coral-xyz/anchor";
import { Decimal } from "decimal.js";
import * as limo from "@kamino-finance/limo-sdk";
import { getPdaAuthority } from "@kamino-finance/limo-sdk/dist/utils";
import { getVersionedTxSize } from "./utils/size";
import { LOOKUP_TABLE_ADDRESS, OPPORTUNITY_WAIT_TIME } from "./const";

const MINUTE_IN_SECS = 60;

export class DexRouter {
  private client: Client;
  private pingTimeout: NodeJS.Timeout | undefined;
  private connectionSvm: Connection;
  private expressRelayConfig?: ExpressRelaySvmConfig;
  private readonly routers: Router[];
  private readonly executor: Keypair;
  private readonly chainId: string;
  private readonly PING_INTERVAL = 30000;

  constructor(
    endpoint: string,
    executor: Keypair,
    chainId: string,
    connectionSvm: Connection
  ) {
    this.client = new Client(
      {
        baseUrl: endpoint,
      },
      undefined,
      this.opportunityHandler.bind(this),
      this.bidStatusHandler.bind(this)
    );
    this.executor = executor;
    this.chainId = chainId;
    this.connectionSvm = connectionSvm;
    this.routers = [
      new JupiterRouter(
        this.chainId,
        this.connectionSvm,
        this.executor.publicKey
      ),
    ];
  }

  async bidStatusHandler(bidStatus: BidStatusUpdate) {
    let resultDetails = "";
    if (bidStatus.type == "submitted" || bidStatus.type == "won") {
      resultDetails = `, transaction ${bidStatus.result}`;
    } else if (bidStatus.type == "lost") {
      if (bidStatus.result) {
        resultDetails = `, transaction ${bidStatus.result}`;
      }
    }
    console.log(
      `Bid status for bid ${bidStatus.id}: ${bidStatus.type}${resultDetails}`
    );
  }

  async opportunityHandler(opportunity: Opportunity) {
    console.log("Received opportunity:", opportunity.opportunityId);
    await new Promise((resolve) => setTimeout(resolve, OPPORTUNITY_WAIT_TIME));
    try {
      const bid = await this.generateRouterBid(opportunity as OpportunitySvm);
      const result = await this.client.requestViaWebsocket({
        method: "post_bid",
        params: {
          bid: bid,
        },
      });
      if (result === null) {
        throw new Error("Empty response in websocket for bid submission");
      }
      console.log(
        `Successful bid. Opportunity id ${opportunity.opportunityId} Bid id ${result.id}`
      );
    } catch (error) {
      console.error(
        `Failed to bid on opportunity ${opportunity.opportunityId}: ${error}`
      );
    }
  }

  async generateRouterBid(opportunity: OpportunitySvm): Promise<{
    transaction: string;
    chain_id: string;
  }> {
    const order = opportunity.order;
    const clientLimo = new limo.LimoClient(
      this.connectionSvm,
      order.state.globalConfig
    );
    const inputMintDecimals = await clientLimo.getOrderInputMintDecimals(order);
    const outputMintDecimals = await clientLimo.getOrderOutputMintDecimals(
      order
    );

    const routerOutputs = (
      await Promise.all(
        this.routers.map(async (router) => {
          try {
            const routerOutput = await router.route(
              opportunity.order.state.inputMint,
              opportunity.order.state.outputMint,
              opportunity.order.state.remainingInputAmount
            );
            return routerOutput;
          } catch (error) {
            console.error(`Failed to route order: ${error}`);
          }
        })
      )
    ).filter((routerOutput) => routerOutput !== undefined);
    if (routerOutputs.length === 0) {
      throw new Error("No routers available to route order");
    }
    const routerBest = routerOutputs.reduce((bestSoFar, curr) => {
      return bestSoFar.amountOut > curr.amountOut ? bestSoFar : curr;
    });
    let ixsRouter = routerBest.ixs;

    const inputAmountDecimals = new Decimal(
      order.state.remainingInputAmount.toNumber()
    ).div(new Decimal(10).pow(inputMintDecimals));
    const outputAmountDecimals = new Decimal(Number(routerBest.amountOut)).div(
      new Decimal(10).pow(outputMintDecimals)
    );

    const ixsFlashTakeOrder = await clientLimo.flashTakeOrderIxs(
      this.executor.publicKey,
      order,
      inputAmountDecimals,
      outputAmountDecimals,
      SVM_CONSTANTS[this.chainId].expressRelayProgram,
      inputMintDecimals,
      outputMintDecimals
    );

    const router = getPdaAuthority(
      clientLimo.getProgramID(),
      order.state.globalConfig
    );
    const bidAmount = new anchor.BN(0);
    if (!this.expressRelayConfig) {
      this.expressRelayConfig = await this.client.getExpressRelaySvmConfig(
        this.chainId,
        this.connectionSvm
      );
    }

    const ixSubmitBid = await this.client.constructSubmitBidInstruction(
      this.executor.publicKey,
      router,
      order.address,
      bidAmount,
      new anchor.BN(Math.round(Date.now() / 1000 + MINUTE_IN_SECS)),
      this.chainId,
      this.expressRelayConfig.relayerSigner,
      this.expressRelayConfig.feeReceiverRelayer
    );

    const txMsg = new TransactionMessage({
      payerKey: this.executor.publicKey,
      recentBlockhash: opportunity.blockHash,
      instructions: [
        ...ixsFlashTakeOrder.createAtaIxs,
        ixsFlashTakeOrder.startFlashIx,
        ixSubmitBid,
        ...ixsRouter,
        ixsFlashTakeOrder.endFlashIx,
        ...ixsFlashTakeOrder.closeWsolAtaIxs,
      ],
    });

    let lookupTableAddresses: PublicKey[] = [];
    if (routerBest.lookupTableAddresses !== undefined) {
      lookupTableAddresses.push(...routerBest.lookupTableAddresses);
    }
    if (LOOKUP_TABLE_ADDRESS[this.chainId] !== undefined) {
      lookupTableAddresses.push(LOOKUP_TABLE_ADDRESS[this.chainId]);
    }
    const lookupTableAccounts = await this.getLookupTableAccounts(
      lookupTableAddresses
    );
    const tx = new VersionedTransaction(
      txMsg.compileToV0Message(lookupTableAccounts)
    );
    tx.sign([this.executor]);

    return {
      transaction: Buffer.from(tx.serialize()).toString("base64"),
      chain_id: this.chainId,
    };
  }

  private async getLookupTableAccounts(
    keys: PublicKey[]
  ): Promise<AddressLookupTableAccount[]> {
    const addressLookupTableAccountInfos = (
      await this.connectionSvm.getMultipleAccountsInfo(
        keys.map((key) => new PublicKey(key))
      )
    ).filter((acc) => acc !== null);

    return addressLookupTableAccountInfos.map((accountInfo, index) => {
      const addressLookupTableAddress = keys[index];
      return new AddressLookupTableAccount({
        key: new PublicKey(addressLookupTableAddress),
        state: AddressLookupTableAccount.deserialize(accountInfo.data),
      });
    });
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
      await this.client.subscribeChains([this.chainId]);
      console.log(
        `Subscribed to chains ${this.chainId}. Waiting for opportunities...`
      );
    } catch (error) {
      console.error(error);
      this.client.websocket?.close();
    }
  }
}

const argv = yargs(hideBin(process.argv))
  .option("endpoint-express-relay", {
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
  .option("chain-id", {
    description: "Chain id to listen and submit routed bids for.",
    type: "string",
    default: "development-solana",
  })
  .option("endpoint-svm", {
    description: "SVM RPC endpoint",
    type: "string",
    demandOption: true,
  })
  .help()
  .alias("help", "h")
  .parseSync();

async function run() {
  const dexRouter = new DexRouter(
    argv["endpoint-express-relay"],
    Keypair.fromSecretKey(anchor.utils.bytes.bs58.decode(argv["sk-executor"])),
    argv["chain-id"],
    new Connection(argv["endpoint-svm"], "confirmed")
  );
  await dexRouter.start();
}

run();
