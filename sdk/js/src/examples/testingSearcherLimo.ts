import { Opportunity, OpportunitySvm } from "../index";
import { SVM_CONSTANTS } from "../const";

import * as anchor from "@coral-xyz/anchor";
import { Keypair } from "@solana/web3.js";
import { OrderStateAndAddress } from "@kamino-finance/limo-sdk/dist/utils";
import {
  getKeypair,
  makeParser,
  SimpleSearcherLimo,
} from "./simpleSearcherLimo";

class SearcherLimo extends SimpleSearcherLimo {
  private fillRate: anchor.BN;

  constructor(
    endpointExpressRelay: string,
    chainId: string,
    searcher: Keypair,
    endpointSvm: string,
    bid: anchor.BN,
    fillRate: number,
    public withLatency: boolean,
    public bidMargin: number,
    public apiKey?: string
  ) {
    super(endpointExpressRelay, chainId, searcher, endpointSvm, bid, apiKey);
    this.fillRate = new anchor.BN(fillRate).div(new anchor.BN(100));
  }

  async getBidAmount(order: OrderStateAndAddress): Promise<anchor.BN> {
    let bidAmount = this.bid;
    if (this.bidMargin !== 0) {
      const margin = new anchor.BN(
        Math.floor(Math.random() * (this.bidMargin * 2 + 1)) - this.bidMargin
      );
      bidAmount = bidAmount.add(margin);
    }
    return bidAmount;
  }

  async opportunityHandler(opportunity: Opportunity) {
    if (!this.recentBlockhash[this.chainId]) {
      console.log(
        `No recent blockhash for chain ${this.chainId}, skipping bid`
      );
      return;
    }

    // todo: factor in fill rate by changing the remaining amount
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

  protected getEffectiveFillRate(order: OrderStateAndAddress): any {
    return anchor.BN.min(this.fillRate, super.getEffectiveFillRate(order));
  }
}

async function run() {
  const argv = makeParser()
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
    .parseSync();
  const searcherKeyPair = getKeypair(argv.privateKey, argv.privateKeyJsonFile);
  const simpleSearcher = new SearcherLimo(
    argv.endpointExpressRelay,
    argv.chainId,
    searcherKeyPair,
    argv.endpointSvm,
    new anchor.BN(argv.bid),
    argv.fillRate,
    argv.withLatency,
    argv.bidMargin,
    argv.apiKey
  );
  await simpleSearcher.start();
}

run();
