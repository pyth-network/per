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
  QuoteTokens,
  SvmChainUpdate,
} from "../types";
import { SVM_CONSTANTS } from "../const";

import * as anchor from "@coral-xyz/anchor";
import {
  Keypair,
  PublicKey,
  Connection,
  TransactionInstruction,
  ComputeBudgetProgram,
} from "@solana/web3.js";

const DAY_IN_SECONDS = 60 * 60 * 24;

export class SimpleSearcherSwap {
  protected client: Client;
  protected readonly connectionSvm: Connection;
  protected mintDecimals: Record<string, number> = {};
  protected expressRelayConfig: ExpressRelaySvmConfig | undefined;
  protected latestChainUpdate: Record<ChainId, SvmChainUpdate> = {};
  protected readonly bid: anchor.BN;
  constructor(
    public endpointExpressRelay: string,
    public chainId: string,
    protected searcher: Keypair,
    public endpointSvm: string,
    bid: number,
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
      this.svmChainUpdateHandler.bind(this)
    );
    this.bid = new anchor.BN(bid);
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

  /**
   * Generates a bid for a given opportunity.
   * The transaction in this bid transfers assets from the searcher's wallet to fulfill the swap request.
   * @param opportunity The SVM opportunity to bid on.
   * @returns The generated bid object.
   */
  async generateBid(opportunity: OpportunitySvm): Promise<BidSvm> {
    if (opportunity.program != "swap") {
      throw new Error("Opportunity is not a swap opportunity");
    }
    // TODO*: implement
    throw new Error("Not implemented");
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
   * Calculates the bid amount for a given swap request.
   * @param tokens The tokens to be swapped
   * @returns The bid amount for the unspecified token
   */
  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  async getBidAmount(tokens: QuoteTokens): Promise<anchor.BN> {
    // this should be replaced by a more sophisticated logic to determine the bid amount
    return this.bid;
  }

  async opportunityHandler(opportunity: Opportunity) {
    if (!this.latestChainUpdate[this.chainId]) {
      console.log(
        `No recent blockhash for chain ${this.chainId}, skipping bid`
      );
      return;
    }
    if ((opportunity as OpportunitySvm).program != "swap") {
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
    this.latestChainUpdate[update.chainId] = update;
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
      type: "number",
      default: 100,
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
          // eslint-disable-next-line @typescript-eslint/no-require-imports
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
  const simpleSearcher = new SimpleSearcherSwap(
    argv.endpointExpressRelay,
    argv.chainId,
    searcherKeyPair,
    argv.endpointSvm,
    argv.bid,
    argv.apiKey
  );
  await simpleSearcher.start();
}

if (require.main === module) {
  run();
}
