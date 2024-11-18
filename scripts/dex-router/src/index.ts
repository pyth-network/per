import {
  BidStatusUpdate,
  ChainId,
  Client,
  ExpressRelaySvmConfig,
  Opportunity,
  OpportunitySvm,
  SVM_CONSTANTS,
  SvmChainUpdate,
} from "@pythnetwork/express-relay-js";
import { Router, RouterOutput, RouterOutputAndTx } from "./types";
import { JupiterRouter } from "./router/jupiter";
import yargs from "yargs";
import { hideBin } from "yargs/helpers";
import {
  AddressLookupTableAccount,
  Blockhash,
  ComputeBudgetProgram,
  Connection,
  Keypair,
  PublicKey,
  TransactionInstruction,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import * as anchor from "@coral-xyz/anchor";
import { Decimal } from "decimal.js";
import * as limo from "@kamino-finance/limo-sdk";
import {
  FlashTakeOrderIxs,
  getMintDecimals,
  getPdaAuthority,
  OrderStateAndAddress,
} from "@kamino-finance/limo-sdk/dist/utils";
import {
  OPPORTUNITY_WAIT_TIME_MS,
  HEALTH_RPC_THRESHOLD,
  HEALTH_EXPRESS_RELAY_INTERVAL,
  HEALTH_RPC_INTERVAL,
  MAX_TX_SIZE,
} from "./const";
import { filterComputeBudgetIxs } from "./utils/computeBudget";
import { checkExpressRelayHealth, checkRpcHealth } from "./utils/health";

const MINUTE_IN_SECS = 60;

export class DexRouter {
  private client: Client;
  private pingTimeout: NodeJS.Timeout | undefined;
  private mintDecimals: Record<string, number> = {};
  private baseLookupTableAddresses: PublicKey[] = [];
  private lookupTableAccounts: Record<string, AddressLookupTableAccount> = {};
  private connectionSvm: Connection;
  private expressRelayConfig?: ExpressRelaySvmConfig;
  private latestChainUpdate: Record<ChainId, SvmChainUpdate> = {};
  private readonly routers: Router[];
  private readonly executor: Keypair;
  private readonly chainId: string;
  private readonly PING_INTERVAL = 30000;

  constructor(
    endpoint: string,
    executor: Keypair,
    chainId: string,
    connectionSvm: Connection,
    maxAccountsJupiter: number[],
    baseLookupTableAddresses?: PublicKey[]
  ) {
    this.client = new Client(
      {
        baseUrl: endpoint,
      },
      undefined,
      this.opportunityHandler.bind(this),
      this.bidStatusHandler.bind(this),
      this.svmChainUpdateHandler.bind(this)
    );
    this.executor = executor;
    this.chainId = chainId;
    this.connectionSvm = connectionSvm;
    this.routers = maxAccountsJupiter.map(
      (maxAccounts) =>
        new JupiterRouter(this.chainId, this.executor.publicKey, maxAccounts)
    );
    this.baseLookupTableAddresses = baseLookupTableAddresses ?? [];
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
    await new Promise((resolve) =>
      setTimeout(resolve, OPPORTUNITY_WAIT_TIME_MS)
    );
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

  async svmChainUpdateHandler(update: SvmChainUpdate) {
    this.latestChainUpdate[update.chain_id] = update;
  }

  async getMintDecimalsCached(mint: PublicKey): Promise<number> {
    const mintAddress = mint.toBase58();
    if (this.mintDecimals[mintAddress]) {
      return this.mintDecimals[mintAddress];
    }
    const decimals = await getMintDecimals(this.connectionSvm, mint);
    this.mintDecimals[mintAddress] = decimals;
    return decimals;
  }

  /**
   * Generates a bid that routes through on-chain liquidity for the provided opportunity
   * @param opportunity The SVM opportunity to generate a bid for
   */
  async generateRouterBid(opportunity: OpportunitySvm): Promise<{
    transaction: string;
    chain_id: string;
  }> {
    const order = opportunity.order;
    const clientLimo = new limo.LimoClient(
      this.connectionSvm,
      order.state.globalConfig
    );

    const routeBest = await this.getBestRoute(order);

    return {
      transaction: Buffer.from(routeBest.tx.serialize()).toString("base64"),
      chain_id: this.chainId,
    };
  }

  /**
   * Creates a full transaction for the provided swap route and order
   * @param route The router output that contains the relevant swap information
   * @param order The order to be fulfilled
   * @returns A VersionedTransaction that can be signed and submitted to the network
   */
  private async createRouterTransaction(
    route: RouterOutput,
    order: OrderStateAndAddress
  ): Promise<VersionedTransaction> {
    const ixsComputeBudget = filterComputeBudgetIxs(route.ixsComputeBudget);
    const ixsRouter = route.ixsRouter;

    const clientLimo = new limo.LimoClient(
      this.connectionSvm,
      order.state.globalConfig
    );
    const ixsFlashTakeOrder = await this.formFlashTakeOrderInstructions(
      clientLimo,
      order,
      Number(route.amountOut)
    );

    const ixSubmitBid = await this.formSubmitBidInstruction(
      order.address,
      order.state.globalConfig,
      clientLimo.getProgramID()
    );

    const tx = await this.formTransaction(
      [
        ...ixsComputeBudget,
        ...ixsFlashTakeOrder.createAtaIxs,
        ixsFlashTakeOrder.startFlashIx,
        ixSubmitBid,
        ...ixsRouter,
        ixsFlashTakeOrder.endFlashIx,
        ...ixsFlashTakeOrder.closeWsolAtaIxs,
      ],
      route.lookupTableAddresses ?? []
    );
    tx.sign([this.executor]);
    if (tx.serialize().length > MAX_TX_SIZE) {
      throw new Error("Transaction exceeds max size");
    }
    return tx;
  }

  /**
   * Examines routes generated by all available routers and returns the one that yields the most output amount. Filters out routes that exceed the max tx size.
   * @param order The order to be fulfilled
   * @returns The best route and the transaction that fulfills the order
   */
  private async getBestRoute(
    order: OrderStateAndAddress
  ): Promise<RouterOutputAndTx> {
    const routerInfos = (
      await Promise.all(
        this.routers.map(async (router) => {
          try {
            const routerOutput = await router.route(
              order.state.inputMint,
              order.state.outputMint,
              order.state.remainingInputAmount
            );
            const routerTx = await this.createRouterTransaction(
              routerOutput,
              order
            );
            return { output: routerOutput, tx: routerTx };
          } catch (error) {
            console.error(`Failed to route order: ${error}`);
          }
        })
      )
    ).filter((routerInfo) => routerInfo !== undefined);
    if (routerInfos.length === 0) {
      throw new Error("No routers available to route order");
    }
    return routerInfos.reduce((bestSoFar, curr) => {
      return bestSoFar.output.amountOut > curr.output.amountOut
        ? bestSoFar
        : curr;
    });
  }

  /**
   * Creates the flash take order instructions on the Limo program
   * @param clientLimo The Limo client
   * @param order The limit order to be fulfilled
   * @param amountOut The amount of the output token to be provided to the maker
   */
  private async formFlashTakeOrderInstructions(
    clientLimo: limo.LimoClient,
    order: OrderStateAndAddress,
    amountOut: number
  ): Promise<FlashTakeOrderIxs> {
    const inputMintDecimals = await this.getMintDecimalsCached(
      order.state.inputMint
    );
    const outputMintDecimals = await this.getMintDecimalsCached(
      order.state.outputMint
    );
    const inputAmountDecimals = new Decimal(
      order.state.remainingInputAmount.toNumber()
    ).div(new Decimal(10).pow(inputMintDecimals));
    const outputAmountDecimals = new Decimal(amountOut).div(
      new Decimal(10).pow(outputMintDecimals)
    );
    return clientLimo.flashTakeOrderIxs(
      this.executor.publicKey,
      order,
      inputAmountDecimals,
      outputAmountDecimals,
      SVM_CONSTANTS[this.chainId].expressRelayProgram,
      inputMintDecimals,
      outputMintDecimals
    );
  }

  /**
   * Creates a 0-SOL bid SubmitBid instruction with the provided permission and router
   * @param permission The permission account to use for the bid
   * @param globalConfig The global config account to use to fetch the router
   * @param limoProgamId The Limo program ID
   */
  private async formSubmitBidInstruction(
    permission: PublicKey,
    globalConfig: PublicKey,
    limoProgamId: PublicKey
  ): Promise<TransactionInstruction> {
    const router = getPdaAuthority(limoProgamId, globalConfig);
    const bidAmount = new anchor.BN(0);
    if (!this.expressRelayConfig) {
      this.expressRelayConfig = await this.client.getExpressRelaySvmConfig(
        this.chainId,
        this.connectionSvm
      );
    }

    return this.client.constructSubmitBidInstruction(
      this.executor.publicKey,
      router,
      permission,
      bidAmount,
      new anchor.BN(Math.round(Date.now() / 1000 + MINUTE_IN_SECS)),
      this.chainId,
      this.expressRelayConfig.relayerSigner,
      this.expressRelayConfig.feeReceiverRelayer
    );
  }

  /**
   * Creates a VersionedTransaction from the provided instructions and lookup table addresses
   * @param instructions The instructions to include in the transaction
   * @param routerLookupTableAddresses The lookup table addresses to include in the transaction
   */
  private async formTransaction(
    instructions: TransactionInstruction[],
    routerLookupTableAddresses: PublicKey[]
  ): Promise<VersionedTransaction> {
    if (!this.latestChainUpdate[this.chainId]) {
      console.log(
        `No recent blockhash for chain ${this.chainId}, getting manually`
      );
      this.latestChainUpdate[this.chainId] = (
        await this.connectionSvm.getLatestBlockhash("confirmed")
      ).blockhash;
    } else {
      const feeInstruction = ComputeBudgetProgram.setComputeUnitPrice({
        microLamports:
          this.latestChainUpdate[this.chainId].latest_prioritization_fee,
      });
      instructions.unshift(feeInstruction);
    }

    const txMsg = new TransactionMessage({
      payerKey: this.executor.publicKey,
      recentBlockhash: this.latestChainUpdate[this.chainId].blockhash,
      instructions,
    });

    const lookupTableAddresses = [
      ...this.baseLookupTableAddresses,
      ...routerLookupTableAddresses,
    ];
    const lookupTableAccounts = await this.getLookupTableAccountsCached(
      lookupTableAddresses
    );
    return new VersionedTransaction(
      txMsg.compileToV0Message(lookupTableAccounts)
    );
  }

  /**
   * Fetches lookup table accounts from the cache. If absent from the cache, fetches them from the network and caches them.
   * @param keys The keys of the lookup table accounts
   */
  private async getLookupTableAccountsCached(
    keys: PublicKey[]
  ): Promise<AddressLookupTableAccount[]> {
    const missingKeys = keys.filter(
      (key) => this.lookupTableAccounts[key.toBase58()] === undefined
    );

    let accountsToReturn = keys
      .filter((key) => !missingKeys.includes(key))
      .map((key) => this.lookupTableAccounts[key.toBase58()]);
    if (missingKeys.length > 0) {
      const missingAccounts = await this.connectionSvm.getMultipleAccountsInfo(
        missingKeys
      );
      missingKeys.forEach((key, index) => {
        if (
          missingAccounts[index] !== null &&
          missingAccounts[index] !== undefined
        ) {
          this.lookupTableAccounts[key.toBase58()] =
            new AddressLookupTableAccount({
              key: key,
              state: AddressLookupTableAccount.deserialize(
                missingAccounts[index].data
              ),
            });
          accountsToReturn.push(this.lookupTableAccounts[key.toBase58()]);
        } else {
          console.warn(
            `Missing lookup table account for key ${key.toBase58()}`
          );
        }
      });
    }

    return accountsToReturn;
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
  .option("lookup-table-addresses", {
    description:
      "Lookup table addresses to include in the submitted transactions. In base58 format.",
    type: "array",
    demandOption: false,
  })
  .option("options-max-accounts-jupiter", {
    description:
      "Options for the max number of accounts to include in Jupiter instructions",
    type: "array",
    default: [10, 20, 30],
  })
  .help()
  .alias("help", "h")
  .parseSync();

async function run() {
  const connection = new Connection(argv["endpoint-svm"], "confirmed");
  const dexRouter = new DexRouter(
    argv["endpoint-express-relay"],
    Keypair.fromSecretKey(anchor.utils.bytes.bs58.decode(argv["sk-executor"])),
    argv["chain-id"],
    connection,
    argv["options-max-accounts-jupiter"].map((maxAccounts) =>
      Number(maxAccounts)
    ),
    argv["lookup-table-addresses"]?.map((address) => new PublicKey(address))
  );
  checkRpcHealth(connection, HEALTH_RPC_THRESHOLD, HEALTH_RPC_INTERVAL).catch(
    console.error
  );
  checkExpressRelayHealth(
    argv["endpoint-express-relay"],
    HEALTH_EXPRESS_RELAY_INTERVAL
  ).catch(console.error);
  await dexRouter.start();
}

run();
