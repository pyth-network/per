import yargs from "yargs";
import { hideBin } from "yargs/helpers";
import fs from "fs";
import {
  Client,
  ExpressRelaySvmConfig,
  Opportunity,
  OpportunitySvm,
  OpportunitySvmLimo,
  OpportunitySvmSwap,
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
  TransactionInstruction,
  ComputeBudgetProgram,
} from "@solana/web3.js";

import * as limo from "@kamino-finance/limo-sdk";
import {
  getMintDecimals,
  getPdaAuthority,
  OrderStateAndAddress,
} from "@kamino-finance/limo-sdk/dist/utils";
import { constructSwapBid } from "../svm";

const DAY_IN_SECONDS = 60 * 60 * 24;

export class SimpleSearcherLimo {
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
      this.svmChainUpdateHandler.bind(this),
      this.removeOpportunitiesHandler.bind(this)
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
   * Generates a bid for a given limo opportunity.
   * The transaction in this bid transfers assets from the searcher's wallet to fulfill the limit order.
   * @param opportunity The SVM opportunity to bid on.
   * @returns The generated bid object.
   */
  async generateBidLimo(opportunity: OpportunitySvmLimo): Promise<BidSvm> {
    const order = opportunity.order;
    const limoClient = new limo.LimoClient(
      this.connectionSvm,
      order.state.globalConfig
    );

    const ixsTakeOrder = await this.generateTakeOrderIxs(limoClient, order);
    const feeInstruction = ComputeBudgetProgram.setComputeUnitPrice({
      microLamports:
        this.latestChainUpdate[this.chainId].latestPrioritizationFee,
    });
    const txRaw = new anchor.web3.Transaction().add(
      feeInstruction,
      ...ixsTakeOrder
    );

    const bidAmount = await this.getBidAmount(opportunity);

    const config = await this.getExpressRelayConfig();
    const bid = await this.client.constructSvmBid(
      txRaw,
      this.searcher.publicKey,
      getPdaAuthority(limoClient.getProgramID(), order.state.globalConfig),
      order.address,
      bidAmount,
      new anchor.BN(Math.round(Date.now() / 1000 + DAY_IN_SECONDS)),
      this.chainId,
      config.relayerSigner,
      config.feeReceiverRelayer
    );
    bid.slot = opportunity.slot;

    bid.transaction.recentBlockhash =
      this.latestChainUpdate[this.chainId].blockhash;
    bid.transaction.sign(this.searcher);
    return bid;
  }

  /**
   * Generates a bid for a given swap opportunity.
   * The transaction in this bid transfers assets from the searcher's wallet to the specified wallets to fulfill the opportunity.
   * @param opportunity The SVM opportunity to bid on.
   * @returns The generated bid object.
   */
  async generateBidSwap(opportunity: OpportunitySvmSwap): Promise<BidSvm> {
    const feeInstruction = ComputeBudgetProgram.setComputeUnitPrice({
      microLamports:
        this.latestChainUpdate[this.chainId].latestPrioritizationFee,
    });
    const bidAmount = await this.getBidAmount(opportunity);
    const txRaw = new anchor.web3.Transaction().add(feeInstruction);
    const config = await this.getExpressRelayConfig();
    const bid = await constructSwapBid(
      txRaw,
      this.searcher.publicKey,
      opportunity,
      bidAmount,
      new anchor.BN(Math.round(Date.now() / 1000 + DAY_IN_SECONDS)),
      this.chainId,
      config.relayerSigner
    );

    bid.transaction.recentBlockhash =
      this.latestChainUpdate[this.chainId].blockhash;
    bid.transaction.feePayer = this.searcher.publicKey;
    bid.transaction.partialSign(this.searcher);
    return bid;
  }

  /**
   * Generates a bid for a given opportunity.
   * The transaction in this bid transfers assets from the searcher's wallet to fulfill the opportunity.
   * @param opportunity The SVM opportunity to bid on.
   * @returns The generated bid object.
   */
  async generateBid(opportunity: OpportunitySvm): Promise<BidSvm> {
    if (opportunity.program === "limo") {
      return this.generateBidLimo(opportunity);
    } else {
      // swap opportunity
      return this.generateBidSwap(opportunity);
    }
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
   * @param opportunity The opportunity to be fulfilled
   * @returns The bid amount in the necessary token
   */
  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  async getBidAmount(opportunity: OpportunitySvm): Promise<anchor.BN> {
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
    const inputAmount = this.getInputAmount(order);
    // take the ceiling of the division by adding order.state.initialInputAmount - 1
    const outputAmount = inputAmount
      .mul(order.state.expectedOutputAmount)
      .add(order.state.initialInputAmount)
      .sub(new anchor.BN(1))
      .div(order.state.initialInputAmount);

    console.log("Order address", order.address.toBase58());
    console.log(
      "Fill rate",
      inputAmount.toNumber() / order.state.initialInputAmount.toNumber()
    );
    console.log(
      "Sell token",
      order.state.inputMint.toBase58(),
      "amount:",
      inputAmount.toNumber() / 10 ** inputMintDecimals
    );
    console.log(
      "Buy token",
      order.state.outputMint.toBase58(),
      "amount:",
      outputAmount.toNumber() / 10 ** outputMintDecimals
    );

    return limoClient.takeOrderIx(
      this.searcher.publicKey,
      order,
      inputAmount,
      outputAmount,
      SVM_CONSTANTS[this.chainId].expressRelayProgram
    );
  }

  protected getInputAmount(order: OrderStateAndAddress): anchor.BN {
    return order.state.remainingInputAmount;
  }

  async opportunityHandler(opportunity: Opportunity) {
    if (!this.latestChainUpdate[this.chainId]) {
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
    this.latestChainUpdate[update.chainId] = update;
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
        Uint8Array.from(
          JSON.parse(fs.readFileSync(privateKeyJsonFile, "utf-8"))
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
    argv.bid,
    argv.apiKey
  );
  await simpleSearcher.start();
}

if (require.main === module) {
  run();
}
