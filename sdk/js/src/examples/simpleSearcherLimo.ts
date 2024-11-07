import yargs from "yargs";
import { hideBin } from "yargs/helpers";
import {
  Client,
  ExpressRelaySvmConfig,
  Opportunity,
  OpportunitySvm,
} from "../index";
import {
  BidStatusUpdate,
  BidSvm,
  ChainId,
  OpportunityDelete,
  SvmChainUpdate,
} from "../types";
import { SVM_CONSTANTS } from "../const";

import * as anchor from "@coral-xyz/anchor";
import {
  Keypair,
  PublicKey,
  Connection,
  Blockhash,
  TransactionInstruction,
} from "@solana/web3.js";

import * as limo from "@kamino-finance/limo-sdk";
import { Decimal } from "decimal.js";
import {
  getMintDecimals,
  getPdaAuthority,
  OrderStateAndAddress,
} from "@kamino-finance/limo-sdk/dist/utils";

const DAY_IN_SECONDS = 60 * 60 * 24;

type BidData = {
  bidAmount: anchor.BN;
  router: PublicKey;
  relayerSigner: PublicKey;
  relayerFeeReceiver: PublicKey;
};

class SimpleSearcherLimo {
  private client: Client;
  private readonly connectionSvm: Connection;
  private mintDecimals: Record<string, number> = {};
  private expressRelayConfig: ExpressRelaySvmConfig | undefined;
  private recentBlockhash: Record<ChainId, Blockhash> = {};
  constructor(
    public endpointExpressRelay: string,
    public chainId: string,
    private searcher: Keypair,
    public endpointSvm: string,
    public fillRate: number,
    public withLatency: boolean,
    public bidMargin: number,
    public apiKey?: string
  ) {
    this.client = new Client(
      {
        baseUrl: endpointExpressRelay,
        apiKey,
      },
      undefined,
      this.opportunityHandler.bind(this),
      this.bidStatusHandler.bind(this),
      this.svmChainUpdateHandler.bind(this),
      this.removeOpportunitiesHandler.bind(this)
    );
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
   * Generates a bid for a given opportunity. The transaction in this bid transfers assets from the searcher's wallet to fulfill the limit order.
   * @param opportunity The SVM opportunity to bid on
   * @returns The generated bid object
   */
  async generateBid(opportunity: OpportunitySvm): Promise<BidSvm> {
    const order = opportunity.order;
    const limoClient = new limo.LimoClient(
      this.connectionSvm,
      order.state.globalConfig
    );

    const ixsTakeOrder = await this.generateTakeOrderIxs(limoClient, order);
    const txRaw = new anchor.web3.Transaction().add(...ixsTakeOrder);

    const bidData = await this.getBidData(limoClient, order);

    const bid = await this.client.constructSvmBid(
      txRaw,
      this.searcher.publicKey,
      bidData.router,
      order.address,
      bidData.bidAmount,
      new anchor.BN(Math.round(Date.now() / 1000 + DAY_IN_SECONDS)),
      this.chainId,
      bidData.relayerSigner,
      bidData.relayerFeeReceiver
    );

    bid.transaction.recentBlockhash = this.recentBlockhash[this.chainId];
    bid.transaction.sign(this.searcher);
    return bid;
  }

  /**
   * Gets the router address, bid amount, and relayer addresses required to create a valid bid
   * @param limoClient The Limo client
   * @param order The limit order to be fulfilled
   * @returns The fetched bid data
   */
  async getBidData(
    limoClient: limo.LimoClient,
    order: OrderStateAndAddress
  ): Promise<BidData> {
    const router = getPdaAuthority(
      limoClient.getProgramID(),
      order.state.globalConfig
    );
    let bidAmount = new anchor.BN(argv.bid);
    if (this.bidMargin !== 0) {
      const margin = new anchor.BN(
        Math.floor(Math.random() * (this.bidMargin * 2 + 1)) - this.bidMargin
      );
      bidAmount = bidAmount.add(margin);
    }
    if (!this.expressRelayConfig) {
      this.expressRelayConfig = await this.client.getExpressRelaySvmConfig(
        this.chainId,
        this.connectionSvm
      );
    }

    return {
      bidAmount,
      router,
      relayerSigner: this.expressRelayConfig.relayerSigner,
      relayerFeeReceiver: this.expressRelayConfig.feeReceiverRelayer,
    };
  }

  /**
   * Creates the take order instructions on the Limo program
   * @param limoClient The Limo client
   * @param order The limit order to be fulfilled
   * @returns The Limo TakeOrder instructions used to fulfill the order
   */
  async generateTakeOrderIxs(
    limoClient: limo.LimoClient,
    order: OrderStateAndAddress
  ): Promise<TransactionInstruction[]> {
    const inputMintDecimals = await this.getMintDecimalsCached(
      order.state.inputMint
    );
    const outputMintDecimals = await this.getMintDecimalsCached(
      order.state.outputMint
    );
    const effectiveFillRate = Math.min(
      this.fillRate,
      (100 * order.state.remainingInputAmount.toNumber()) /
        order.state.initialInputAmount.toNumber()
    );
    const inputAmountDecimals = new Decimal(
      order.state.initialInputAmount.toNumber()
    )
      .mul(effectiveFillRate)
      .div(100)
      .floor()
      .div(new Decimal(10).pow(inputMintDecimals));

    const outputAmountDecimals = new Decimal(
      order.state.expectedOutputAmount.toNumber()
    )
      .mul(effectiveFillRate)
      .div(100)
      .ceil()
      .div(new Decimal(10).pow(outputMintDecimals));

    console.log("Order address", order.address.toBase58());
    console.log("Fill rate", effectiveFillRate);
    console.log(
      "Sell token",
      order.state.inputMint.toBase58(),
      "amount:",
      inputAmountDecimals.toString()
    );
    console.log(
      "Buy token",
      order.state.outputMint.toBase58(),
      "amount:",
      outputAmountDecimals.toString()
    );

    return limoClient.takeOrderIx(
      this.searcher.publicKey,
      order,
      inputAmountDecimals,
      outputAmountDecimals,
      SVM_CONSTANTS[this.chainId].expressRelayProgram,
      inputMintDecimals,
      outputMintDecimals
    );
  }

  async opportunityHandler(opportunity: Opportunity) {
    if (!this.recentBlockhash[this.chainId]) {
      console.log(
        `No recent blockhash for chain ${this.chainId}, skipping bid`
      );
      return;
    }

    if (this.withLatency) {
      const latency = Math.floor(Math.random() * 500);
      console.log(`Adding latency of ${latency}ms`);
      await new Promise((resolve) => setTimeout(resolve, latency));
    }
    const bid = await this.generateBid(opportunity as OpportunitySvm);
    try {
      const bidId = await this.client.submitBid(bid);
      console.log(
        `Successful bid. Opportunity id ${opportunity.opportunityId} Bid id ${bidId}`
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

  // NOTE: Developers are responsible for implementing custom removal logic specific to their use case.
  async removeOpportunitiesHandler(opportunityDelete: OpportunityDelete) {
    console.log(
      `Opportunities ${JSON.stringify(opportunityDelete)} don't exist anymore`
    );
  }

  async start() {
    try {
      await this.client.subscribeChains([argv.chainId]);
      console.log(
        `Subscribed to chain ${argv.chainId}. Waiting for opportunities...`
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
    demandOption: true,
  })
  .option("chain-id", {
    description: "Chain id to bid on Limo opportunities for. e.g: solana",
    type: "string",
    demandOption: true,
  })
  .option("bid", {
    description: "Bid amount in lamports",
    type: "string",
    default: "100",
  })
  .option("private-key", {
    description: "Private key of the searcher in base58 format",
    type: "string",
    conflicts: "private-key-json-file",
  })
  .option("private-key-json-file", {
    description:
      "Path to a json file containing the private key of the searcher in array of bytes format",
    type: "string",
    conflicts: "private-key",
  })
  .option("api-key", {
    description:
      "The API key of the searcher to authenticate with the server for fetching and submitting bids",
    type: "string",
    demandOption: false,
  })
  .option("endpoint-svm", {
    description: "SVM RPC endpoint",
    type: "string",
    demandOption: true,
  })
  .option("fill-rate", {
    description:
      "How much of the initial order size to fill in percentage. Default is 100%",
    type: "number",
    default: 100,
  })
  .option("with-latency", {
    description:
      "Whether to add random latency to the bid submission. Default is false",
    type: "boolean",
    default: false,
  })
  .option("bid-margin", {
    description:
      "The margin to add or subtract from the bid. For example, 1 means the bid range is [bid - 1, bid + 1]. Default is 0",
    type: "number",
    default: 0,
  })
  .help()
  .alias("help", "h")
  .parseSync();
async function run() {
  if (!SVM_CONSTANTS[argv.chainId]) {
    throw new Error(`SVM constants not found for chain ${argv.chainId}`);
  }
  let searcherKeyPair;

  if (argv.privateKey) {
    const secretKey = anchor.utils.bytes.bs58.decode(argv.privateKey);
    searcherKeyPair = Keypair.fromSecretKey(secretKey);
  } else if (argv.privateKeyJsonFile) {
    searcherKeyPair = Keypair.fromSecretKey(
      Buffer.from(
        // eslint-disable-next-line @typescript-eslint/no-var-requires
        JSON.parse(require("fs").readFileSync(argv.privateKeyJsonFile))
      )
    );
  } else {
    throw new Error(
      "Either private-key or private-key-json-file must be provided"
    );
  }
  console.log(`Using searcher pubkey: ${searcherKeyPair.publicKey.toBase58()}`);

  const simpleSearcher = new SimpleSearcherLimo(
    argv.endpointExpressRelay,
    argv.chainId,
    searcherKeyPair,
    argv.endpointSvm,
    argv.fillRate,
    argv.withLatency,
    argv.bidMargin,
    argv.apiKey
  );
  await simpleSearcher.start();
}

run();
