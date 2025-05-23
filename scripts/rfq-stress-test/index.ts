import yargs from "yargs";
import { hideBin } from "yargs/helpers";
import _ from "lodash";
import { Client, ClientError } from "@pythnetwork/express-relay-js";
import { PublicKey } from "@solana/web3.js";
import fs from "fs/promises";

const argv = yargs(hideBin(process.argv))
  .option("public-key", {
    description: "Public key to ask quote for in base58 format",
    type: "string",
    default: "H8sMJSCQxfKiFTCfDR3DUMLPwcRbM61LGFJ8N4dK3WjS",
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

type TaskResult = {
  latency: number;
  status: string;
};

async function loadTokensFromFile(): Promise<PublicKey[]> {
  try {
    const fileContents = await fs.readFile("./tokens.json", "utf-8");
    const tokens = JSON.parse(fileContents);

    // Convert strings into PublicKey objects
    return tokens.map((token: string) => new PublicKey(token));
  } catch (error) {
    console.error(
      `Error reading or parsing tokens.json file at ./tokens.json: (falling back to default tokens)`,
      (error as Error).message,
    );

    return TOKENS;
  }
}

async function runBatches(
  url: string,
  chainId: string,
  publicKey: PublicKey,
  concurrency: number,
  throughput: number,
  availableTokens: PublicKey[],
): Promise<TaskResult[]> {
  const client = new Client({
    baseUrl: url,
  });
  const indices = _.range(throughput);
  const batches = _.chunk(indices, concurrency);

  const result: TaskResult[] = [];
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
          const tokens = _.sampleSize(availableTokens, 2);
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

    result.push(...(await Promise.all(wrappedTasks)));

    // Ensure 1-second delay before next batch
    const elapsedTime = Date.now() - startTime;
    if (elapsedTime < 1000) {
      await new Promise((resolve) => setTimeout(resolve, 1000 - elapsedTime));
    }
  }

  return result;
}

const LATENCY_PERCENTILES = [0.5, 0.9, 0.95, 0.99];

function getLatencyPercentile(latency: number[], percentile: number): number {
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

  const tokens = await loadTokensFromFile();
  const result = await runBatches(
    serverUrl,
    argv["chain-id"],
    publicKey,
    concurrency,
    throughput,
    tokens,
  );
  const latency = result.map((task) => task.latency);
  const status = result.map((task) => task.status);

  for (const percentile of LATENCY_PERCENTILES) {
    console.log(
      `P${percentile * 100} latency: ${getLatencyPercentile(
        latency,
        percentile,
      )} ms`,
    );
  }

  const statusCounts = _.countBy(status);
  console.log("Status counts: ", statusCounts);
}

run();
