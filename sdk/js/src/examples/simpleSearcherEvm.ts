import yargs from "yargs";
import { hideBin } from "yargs/helpers";
import { BidStatusUpdateEvm, checkHex, Client } from "../index";
import { privateKeyToAccount } from "viem/accounts";
import { isHex } from "viem";
import { BidStatusUpdate, Opportunity } from "../types";
import { OPPORTUNITY_ADAPTER_CONFIGS } from "../const";

const DAY_IN_SECONDS = 60 * 60 * 24;

class SimpleSearcherEvm {
  private client: Client;
  constructor(
    public endpoint: string,
    public chainId: string,
    public privateKey: string,
    public apiKey?: string,
  ) {
    this.client = new Client(
      {
        baseUrl: endpoint,
        apiKey,
      },
      undefined,
      this.opportunityHandler.bind(this),
      this.bidStatusHandler.bind(this),
      undefined,
      undefined,
      this.websocketCloseHandler.bind(this),
    );
  }

  async websocketCloseHandler() {
    console.log("Websocket closed. Exiting...");
    process.exit(1);
  }

  async bidStatusHandler(_bidStatus: BidStatusUpdate) {
    const bidStatus = _bidStatus as BidStatusUpdateEvm;
    let resultDetails = "";
    if (bidStatus.type == "submitted" || bidStatus.type == "won") {
      resultDetails = `, transaction ${bidStatus.result}, index ${bidStatus.index} of multicall`;
    } else if (bidStatus.type == "lost") {
      if (bidStatus.result) {
        resultDetails = `, transaction ${bidStatus.result}`;
      }
      if (bidStatus.index) {
        resultDetails += `, index ${bidStatus.index} of multicall`;
      }
    }
    console.log(
      `Bid status for bid ${bidStatus.id}: ${bidStatus.type}${resultDetails}`,
    );
  }

  async opportunityHandler(opportunity: Opportunity) {
    if (!("targetContract" in opportunity))
      throw new Error("Not a valid EVM opportunity");
    const bidAmount = BigInt(argv.bid);
    // Bid info should be generated by evaluating the opportunity
    // here for simplicity we are using a constant bid and 24 hours of validity
    // TODO: generate nonce more intelligently, to reduce gas costs
    const nonce = BigInt(Math.floor(Math.random() * 2 ** 50));
    const bidParams = {
      amount: bidAmount,
      nonce: nonce,
      deadline: BigInt(Math.round(Date.now() / 1000 + DAY_IN_SECONDS)),
    };
    const bid = await this.client.signBid(
      opportunity,
      bidParams,
      checkHex(argv.privateKey),
    );
    try {
      const bidId = await this.client.submitBid(bid);
      console.log(
        `Successful bid. Opportunity id ${opportunity.opportunityId} Bid id ${bidId}`,
      );
    } catch (error) {
      console.error(
        `Failed to bid on opportunity ${opportunity.opportunityId}: ${error}`,
      );
    }
  }

  async start() {
    try {
      await this.client.subscribeChains([argv.chainId]);
      console.log(
        `Subscribed to chain ${argv.chainId}. Waiting for opportunities...`,
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
    demandOption: true,
  })
  .option("chain-id", {
    description: "Chain id to fetch opportunities for. e.g: sepolia",
    type: "string",
    demandOption: true,
  })
  .option("bid", {
    description: "Bid amount in wei",
    type: "string",
    default: "10000000000000000",
  })
  .option("private-key", {
    description:
      "Private key to sign the bid with in hex format with 0x prefix. e.g: 0xdeadbeef...",
    type: "string",
    demandOption: true,
  })
  .option("api-key", {
    description:
      "The API key of the searcher to authenticate with the server for fetching and submitting bids",
    type: "string",
    demandOption: false,
  })
  .help()
  .alias("help", "h")
  .parseSync();
async function run() {
  if (isHex(argv.privateKey)) {
    const account = privateKeyToAccount(argv.privateKey);
    console.log(`Using account: ${account.address}`);
  } else {
    throw new Error(`Invalid private key: ${argv.privateKey}`);
  }
  const searcher = new SimpleSearcherEvm(
    argv.endpoint,
    argv.chainId,
    argv.privateKey,
    argv.apiKey,
  );
  if (OPPORTUNITY_ADAPTER_CONFIGS[argv.chainId] === undefined) {
    throw new Error(
      `Opportunity adapter config not found for chain ${argv.chainId}`,
    );
  }
  await searcher.start();
}

run();
