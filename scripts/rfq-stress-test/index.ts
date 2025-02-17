import yargs from "yargs";
import { hideBin } from "yargs/helpers";
import _ from "lodash";
import { Client, ClientError } from "@pythnetwork/express-relay-js";
import { PublicKey } from "@solana/web3.js";

const argv = yargs(hideBin(process.argv))
  .option("public-key", {
    description: "Public key to ask quote for in base58 format",
    type: "string",
    demandOption: true,
  })
  .option("chain-id", {
    description: "The chain ID to get quotes for",
    type: "string",
    demandOption: true,
  })
  .option("server-url", {
    description: "The URL of the auction server",
    type: "string",
    demandOption: true,
  })
  .option("concurrency", {
    description: "Number of concurrent requests per second",
    type: "number",
    default: 10,
  })
  .option("throughput", {
    description: "The total number of requests sent over the test duration",
    type: "number",
    default: 100,
  })
  .help()
  .alias("help", "h")
  .parseSync();

const TOKENS = [
  // Wrapped Sol
  new PublicKey("So11111111111111111111111111111111111111112"),
  // USDC
  new PublicKey("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"),
  // USDT
  new PublicKey("Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB"),
];

const latency: number[] = [];
const status: string[] = [];

type TaskResult = {
  latency: number;
  status: string;
};

async function runBatches(
  url: string,
  chainId: string,
  publicKey: PublicKey,
  concurrency: number,
  throughput: number,
) {
  const client = new Client({
    baseUrl: url,
  });
  const indices = _.range(throughput);
  const batches = _.chunk(indices, concurrency);

  for (let index = 0; index < batches.length; index++) {
    const startTime = Date.now();
    const batch = batches[index];

    console.log(
      `Batch ${index + 1} started: `,
      new Date(startTime).toLocaleString(),
    );
    const wrappedTasks: Promise<TaskResult>[] = batch.map(
      async (index: number) => {
        const taskStart = Date.now();

        const taskPromise: Promise<TaskResult> = new Promise((resolve) => {
          const tokens = _.sampleSize(TOKENS, 2);
          const shuffledTokens = _.shuffle(tokens);
          client
            .getQuote({
              chainId,
              inputTokenMint: shuffledTokens[0],
              outputTokenMint: shuffledTokens[1],
              specifiedTokenAmount: {
                amount: index + 100,
                side: _.sample(["input", "output"]),
              },
              userWallet: publicKey,
            })
            .then(() => {
              resolve({ latency: Date.now() - taskStart, status: "200" });
            })
            .catch((error: unknown) => {
              if (error instanceof ClientError) {
                const cleanedMessage = error.message
                  .replace("Auction server http error", "")
                  .trim();
                const status = cleanedMessage.split(" ")[0];
                resolve({ latency: Date.now() - taskStart, status });
              }
              resolve({ latency: Date.now() - taskStart, status: "unknown" });
            });
        });

        const timeoutPromise = new Promise((_, reject) =>
          setTimeout(() => reject(new Error("Time limit exceeded")), 1000),
        );

        try {
          const result = (await Promise.race([
            taskPromise,
            timeoutPromise,
          ])) as TaskResult;
          return result;
        } catch {
          return { latency: Date.now() - taskStart, status: "timeout" };
        }
      },
    );

    const results = await Promise.all(wrappedTasks);
    results.forEach((result) => {
      latency.push(result.latency);
      status.push(result.status);
    });

    // Ensure 1-second delay before next batch
    const elapsedTime = Date.now() - startTime;
    if (elapsedTime < 1000) {
      await new Promise((resolve) => setTimeout(resolve, 1000 - elapsedTime));
    }
  }
}

const LATENCY_PERCENTILES = [0.5, 0.9, 0.95, 0.99];

function getLatencyPercentile(percentile: number): number {
  const sorted = _.sortBy(latency);
  const index = Math.ceil(percentile * sorted.length) - 1;
  return sorted[Math.max(index, 0)];
}

async function run() {
  const { serverUrl, concurrency, throughput } = argv;
  const publicKey = new PublicKey(argv["public-key"]);

  console.log("--------------------------------------------------");
  console.log("Starting RFQ stress test with the following parameters:");
  console.log(`- Public key: ${publicKey.toBase58()}`);
  console.log(`- Server URL: ${serverUrl}`);
  console.log(`- Concurrency: ${concurrency.toLocaleString()}`);
  console.log(`- Throughput: ${throughput.toLocaleString()}`);
  console.log("--------------------------------------------------");

  await runBatches(
    serverUrl,
    argv["chain-id"],
    publicKey,
    concurrency,
    throughput,
  );

  console.log(latency);

  for (const percentile of LATENCY_PERCENTILES) {
    console.log(
      `P${percentile * 100} latency: ${getLatencyPercentile(percentile)} ms`,
    );
  }

  const statusCounts = _.countBy(status);
  console.log("Status counts: ", statusCounts);
}

run();
