import { Opportunity } from "../index";

import * as anchor from "@coral-xyz/anchor";
import { Keypair } from "@solana/web3.js";
import { OrderStateAndAddress } from "@kamino-finance/limo-sdk/dist/utils";
import { getKeypair, makeParser, SimpleSearcherSvm } from "./simpleSearcherSvm";

class SearcherLimo extends SimpleSearcherSvm {
  private readonly fillRate: anchor.BN;

  constructor(
    endpointExpressRelay: string,
    chainId: string,
    searcher: Keypair,
    endpointSvm: string,
    bid: number,
    fillRate: number,
    public withLatency: boolean,
    public bidMargin: number,
    public apiKey?: string,
  ) {
    super(endpointExpressRelay, chainId, searcher, endpointSvm, bid, apiKey);
    this.fillRate = new anchor.BN(fillRate);
  }

  async getBidAmount(): Promise<anchor.BN> {
    const margin = new anchor.BN(
      Math.floor(Math.random() * (this.bidMargin * 2 + 1)) - this.bidMargin,
    );
    return this.bid.add(margin);
  }

  async opportunityHandler(opportunity: Opportunity): Promise<void> {
    if (this.withLatency) {
      const latency = Math.floor(Math.random() * 300);
      console.log(`Adding latency of ${latency}ms`);
      await new Promise((resolve) => setTimeout(resolve, latency));
    }
    return super.opportunityHandler(opportunity);
  }

  protected getInputAmount(order: OrderStateAndAddress): anchor.BN {
    return anchor.BN.min(
      super.getInputAmount(order),
      order.state.initialInputAmount.mul(this.fillRate).div(new anchor.BN(100)),
    );
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
    argv.bid,
    argv.fillRate,
    argv.withLatency,
    argv.bidMargin,
    argv.apiKey,
  );
  await simpleSearcher.start();
}

if (require.main === module) {
  run();
}
