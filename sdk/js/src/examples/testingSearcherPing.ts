import * as anchor from "@coral-xyz/anchor";
import { ComputeBudgetProgram, Keypair } from "@solana/web3.js";
import { getKeypair, makeParser, SimpleSearcherSvm } from "./simpleSearcherSvm";

class SearcherPinger extends SimpleSearcherSvm {
  constructor(
    endpointExpressRelay: string,
    chainId: string,
    searcher: Keypair,
    endpointSvm: string,
    bid: number,
    public apiKey?: string
  ) {
    super(endpointExpressRelay, chainId, searcher, endpointSvm, bid, apiKey);
  }

  async opportunityHandler(): Promise<void> {
    // don't do anything with the opportunity
  }

  async ping() {
    if (!this.latestChainUpdate[this.chainId]) {
      console.log(
        `No recent blockhash for chain ${this.chainId}, skipping ping`
      );
      return;
    }
    const feeInstruction = ComputeBudgetProgram.setComputeUnitPrice({
      microLamports:
        this.latestChainUpdate[this.chainId].latestPrioritizationFee,
    });
    const config = await this.getExpressRelayConfig();
    const txRaw = new anchor.web3.Transaction().add(feeInstruction);
    const bid = await this.client.constructSvmBid(
      txRaw,
      this.searcher.publicKey,
      this.searcher.publicKey,
      this.searcher.publicKey,
      this.bid,
      new anchor.BN(Math.round(Date.now() / 1000 + 60)),
      this.chainId,
      config.relayerSigner,
      config.feeReceiverRelayer
    );
    bid.transaction.recentBlockhash =
      this.latestChainUpdate[this.chainId].blockhash;
    bid.transaction.sign(this.searcher);
    try {
      const bidId = await this.client.submitBid(bid);
      console.log(`Successful bid. Bid id ${bidId}`);
    } catch (error) {
      console.error(`Failed to ping: ${error}`);
    }
  }

  async start() {
    // run ping every 10 seconds
    setInterval(async () => {
      await this.ping();
    }, 10000);

    await super.start();
  }
}

async function run() {
  const argv = makeParser().parseSync();
  const searcherKeyPair = getKeypair(argv.privateKey, argv.privateKeyJsonFile);
  const simpleSearcher = new SearcherPinger(
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
