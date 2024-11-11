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

export class SimpleSearcherLimo {
  protected client: Client;
  protected readonly connectionSvm: Connection;
  protected mintDecimals: Record<string, number> = {};
  protected expressRelayConfig: ExpressRelaySvmConfig | undefined;
  protected recentBlockhash: Record<ChainId, Blockhash> = {};
  constructor(
    public endpointExpressRelay: string,
    public chainId: string,
    protected searcher: Keypair,
    public endpointSvm: string,
    public bid: anchor.BN,
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

    const bidAmount = await this.getBidAmount(order);

    const bid = await this.client.constructSvmBid(
      txRaw,
      this.searcher.publicKey,
      getPdaAuthority(limoClient.getProgramID(), order.state.globalConfig),
      order.address,
      bidAmount,
      new anchor.BN(Math.round(Date.now() / 1000 + DAY_IN_SECONDS)),
      this.chainId,
      (
        await this.getExpressRelayConfig()
      ).relayerSigner,
      (
        await this.getExpressRelayConfig()
      ).feeReceiverRelayer
    );

    bid.transaction.recentBlockhash = this.recentBlockhash[this.chainId];
    bid.transaction.sign(this.searcher);
    return bid;
  }

  async getExpressRelayConfig(): Promise<ExpressRelaySvmConfig> {
    if (!this.expressRelayConfig) {
      this.expressRelayConfig = await this.client.getExpressRelaySvmConfig(
        this.chainId,
        this.connectionSvm
      );
    }
    return this.expressRelayConfig;
  }

  /**
   * Calculates the bid amount for a given order.
   * @param order The limit order to be fulfilled
   * @returns The bid amount in lamports
   */
  async getBidAmount(order: OrderStateAndAddress): Promise<anchor.BN> {
    // this should be replaced by a more sophisticated logic to determine the bid amount
    return this.bid;
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
    const effectiveFillRate = this.getEffectiveFillRate(order);
    const inputAmountDecimals = new Decimal(
      order.state.initialInputAmount.toNumber()
    )
      .mul(effectiveFillRate)
      .floor()
      .div(new Decimal(10).pow(inputMintDecimals));

    const outputAmountDecimals = new Decimal(
      order.state.expectedOutputAmount.toNumber()
    )
      .mul(effectiveFillRate)
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

  protected getEffectiveFillRate(order: OrderStateAndAddress): Decimal {
    return new Decimal(order.state.remainingInputAmount.toNumber()).div(
      new Decimal(order.state.initialInputAmount.toNumber())
    );
  }

  async opportunityHandler(opportunity: Opportunity) {
    if (!this.recentBlockhash[this.chainId]) {
      console.log(
        `No recent blockhash for chain ${this.chainId}, skipping bid`
      );
      return;
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
    console.log(`Using searcher pubkey: ${this.searcher.publicKey.toBase58()}`);
    try {
      await this.client.subscribeChains([this.chainId]);
      console.log(
        `Subscribed to chain ${this.chainId}. Waiting for opportunities...`
      );
    } catch (error) {
      console.error(error);
      this.client.websocket?.close();
    }
  }
}

export function makeParser() {
  return yargs(hideBin(process.argv))
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
      choices: Object.keys(SVM_CONSTANTS),
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
    .help()
    .alias("help", "h");
}

export function getKeypair(
  privateKey: string | undefined,
  privateKeyJsonFile: string | undefined
): Keypair {
  if (privateKey) {
    const secretKey = anchor.utils.bytes.bs58.decode(privateKey);
    return Keypair.fromSecretKey(secretKey);
  } else {
    if (privateKeyJsonFile) {
      return Keypair.fromSecretKey(
        Buffer.from(
          // eslint-disable-next-line @typescript-eslint/no-var-requires
          JSON.parse(require("fs").readFileSync(privateKeyJsonFile))
        )
      );
    } else {
      throw new Error(
        "Either private-key or private-key-json-file must be provided"
      );
    }
  }
}

async function run() {
  const argv = makeParser().parseSync();
  const searcherKeyPair = getKeypair(argv.privateKey, argv.privateKeyJsonFile);
  const simpleSearcher = new SimpleSearcherLimo(
    argv.endpointExpressRelay,
    argv.chainId,
    searcherKeyPair,
    argv.endpointSvm,
    new anchor.BN(argv.bid),
    argv.apiKey
  );
  await simpleSearcher.start();
}

if (require.main === module) {
  run();
}
