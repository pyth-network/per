import {
  BidStatusUpdate,
  Client,
  ExpressRelaySvmConfig,
  Opportunity,
  OpportunitySvm,
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
    skExecutor: string,
    chainId: string,
    endpointSvm: string
  ) {
    this.client = new Client(
      {
        baseUrl: endpoint,
      },
      undefined,
      this.opportunityHandler.bind(this),
      this.bidStatusHandler.bind(this)
    );
    this.routers = [new JupiterRouter()];
    this.executor = Keypair.fromSecretKey(
      anchor.utils.bytes.bs58.decode(skExecutor)
    );
    this.chainId = chainId;
    this.connectionSvm = new Connection(endpointSvm, "confirmed");
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
      // const result = await this.client.requestViaWebsocket({
      //   method: "post_bid",
      //   params: {
      //     bid: bid,
      //   },
      // });
      // if (result === null) {
      //   throw new Error("Empty response in websocket for bid submission");
      // }
      // console.log(
      //   `Successful bid. Opportunity id ${opportunity.opportunityId} Bid id ${result.id}`
      // );
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

    let routerOutputs: RouterOutput[] = [];
    for (const router of this.routers) {
      try {
        const routerOutput = await router.route(
          opportunity.chainId,
          opportunity.order.state.inputMint,
          opportunity.order.state.outputMint,
          opportunity.order.state.remainingInputAmount,
          this.executor.publicKey
        );
        routerOutputs.push(routerOutput);
      } catch (error) {
        console.error(`Failed to route order: ${error}`);
      }
    }
    if (routerOutputs.length === 0) {
      throw new Error("No routers available to route order");
    }
    const routerBest = routerOutputs.reduce((prev, curr) => {
      return prev.amountOut > curr.amountOut ? prev : curr;
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
      new PublicKey("PytERJFhAKuNNuaiXkApLfWzwNwSNDACpigT3LwQfou"),
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
      new anchor.BN(Math.round(Date.now() / 1000 + 60)),
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
    console.log("length of lookupTableAddresses", lookupTableAddresses.length);
    // TODO: this won't work until we fix account lookup bug in the auction server
    if (LOOKUP_TABLE_ADDRESS[this.chainId] !== undefined) {
      lookupTableAddresses.push(LOOKUP_TABLE_ADDRESS[this.chainId]);
    }
    console.log("length of lookupTableAddresses", lookupTableAddresses.length);
    console.log("lookupTableAddresses", lookupTableAddresses);
    const lookupTableAccounts = await this.getLookupTableAccounts(
      lookupTableAddresses
    );
    const tx = new VersionedTransaction(
      txMsg.compileToV0Message(lookupTableAccounts)
    );
    console.log("header");
    console.log(tx.message.header);
    console.log(
      "static account keys",
      "length",
      tx.message.staticAccountKeys.length
    );
    console.log(tx.message.staticAccountKeys);
    console.log("recent blockhash");
    console.log(tx.message.recentBlockhash);
    console.log(
      "compiled ixs",
      "length",
      tx.message.compiledInstructions.length
    );
    console.log(tx.message.compiledInstructions);
    console.log(
      "address table lookups",
      "length",
      tx.message.addressTableLookups.length
    );
    console.log(tx.message.addressTableLookups);
    console.log("END");
    tx.sign([this.executor]);

    return {
      transaction: Buffer.from(tx.serialize()).toString("base64"),
      chain_id: this.chainId,
    };
  }

  private async getLookupTableAccounts(
    keys: PublicKey[]
  ): Promise<AddressLookupTableAccount[]> {
    const addressLookupTableAccountInfos =
      await this.connectionSvm.getMultipleAccountsInfo(
        keys.map((key) => new PublicKey(key))
      );

    return addressLookupTableAccountInfos.reduce((acc, accountInfo, index) => {
      const addressLookupTableAddress = keys[index];
      if (accountInfo) {
        const addressLookupTableAccount = new AddressLookupTableAccount({
          key: new PublicKey(addressLookupTableAddress),
          state: AddressLookupTableAccount.deserialize(accountInfo.data),
        });
        acc.push(addressLookupTableAccount);
      }

      return acc;
    }, new Array<AddressLookupTableAccount>());
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
    argv["sk-executor"],
    argv["chain-id"],
    argv["endpoint-svm"]
  );
  await dexRouter.start();
}

run();
