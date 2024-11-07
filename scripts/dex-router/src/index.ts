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
import { Router, RouterOutput } from "./types";
import { JupiterRouter } from "./router/jupiter";
import yargs from "yargs";
import { hideBin } from "yargs/helpers";
import {
  AddressLookupTableAccount,
  Blockhash,
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
import { OPPORTUNITY_WAIT_TIME_MS } from "./const";
import { filterComputeBudgetIxs } from "./utils/computeBudget";

const MINUTE_IN_SECS = 60;

export class DexRouter {
  private client: Client;
  private pingTimeout: NodeJS.Timeout | undefined;
  private mintDecimals: Record<string, number> = {};
  private baseLookupTableAddresses: PublicKey[] = [];
  private lookupTableAccounts: Record<string, AddressLookupTableAccount> = {};
  private connectionSvm: Connection;
  private expressRelayConfig?: ExpressRelaySvmConfig;
  private recentBlockhash: Record<ChainId, Blockhash> = {};
  private readonly routers: Router[];
  private readonly executor: Keypair;
  private readonly chainId: string;
  private readonly PING_INTERVAL = 30000;

  constructor(
    endpoint: string,
    executor: Keypair,
    chainId: string,
    connectionSvm: Connection,
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
    this.routers = [new JupiterRouter(this.chainId, this.executor.publicKey)];
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
    this.recentBlockhash[update.chain_id] = update.blockhash;
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

    // TODO: refactor to filter out routes with transactions that exceed the max transaction size
    const routeBest = await this.getBestRoute(
      order.state.inputMint,
      order.state.outputMint,
      order.state.remainingInputAmount
    );
    const ixsComputeBudget = filterComputeBudgetIxs(routeBest.ixsComputeBudget);
    const ixsRouter = routeBest.ixsRouter;

    const ixsFlashTakeOrder = await this.formFlashTakeOrderInstructions(
      clientLimo,
      order,
      Number(routeBest.amountOut)
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
      routeBest.lookupTableAddresses ?? []
    );
    tx.sign([this.executor]);

    return {
      transaction: Buffer.from(tx.serialize()).toString("base64"),
      chain_id: this.chainId,
    };
  }

  /**
   * Examines routes generated by all available routers and returns the one that yields the most output amount
   * @param inputMint The mint of the token to be sold through the router
   * @param outputMint The mint of the token to be bought through the router
   * @param amountIn The amount of the input token to be sold
   */
  private async getBestRoute(
    inputMint: PublicKey,
    outputMint: PublicKey,
    amountIn: bigint
  ): Promise<RouterOutput> {
    const routerOutputs = (
      await Promise.all(
        this.routers.map(async (router) => {
          try {
            const routerOutput = await router.route(
              inputMint,
              outputMint,
              amountIn
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
    return routerOutputs.reduce((bestSoFar, curr) => {
      return bestSoFar.amountOut > curr.amountOut ? bestSoFar : curr;
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
    if (!this.recentBlockhash[this.chainId]) {
      console.log(
        `No recent blockhash for chain ${this.chainId}, getting manually`
      );
      this.recentBlockhash[this.chainId] = (
        await this.connectionSvm.getLatestBlockhash("confirmed")
      ).blockhash;
    }

    const txMsg = new TransactionMessage({
      payerKey: this.executor.publicKey,
      recentBlockhash: this.recentBlockhash[this.chainId],
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
  .help()
  .alias("help", "h")
  .parseSync();

async function run() {
  const dexRouter = new DexRouter(
    argv["endpoint-express-relay"],
    Keypair.fromSecretKey(anchor.utils.bytes.bs58.decode(argv["sk-executor"])),
    argv["chain-id"],
    new Connection(argv["endpoint-svm"], "confirmed"),
    argv["lookup-table-addresses"]?.map((address) => new PublicKey(address))
  );
  await dexRouter.start();
}

run();
