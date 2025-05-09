import yargs from "yargs";
import { hideBin } from "yargs/helpers";
import _ from "lodash";
import { Client } from "@pythnetwork/express-relay-js";
import { PublicKey } from "@solana/web3.js";
import Decimal from "decimal.js";

const argv = yargs(hideBin(process.argv))
  .option("public-key", {
    description: "Public key to ask quote for in base58 format",
    type: "string",
    default: "H8sMJSCQxfKiFTCfDR3DUMLPwcRbM61LGFJ8N4dK3WjS",
  })
  .option("chain-id", {
    description: "The chain ID to get quotes for",
    type: "string",
    default: "solana",
  })
  .option("server-url", {
    description: "The URL of the auction server",
    type: "string",
    default: "https://per-mainnet.dourolabs.app",
  })
  .help()
  .alias("help", "h")
  .parseSync();

type Token = {
  mint: PublicKey;
  decimals: number;
};

const TOKENS: Token[] = [
  // Wrapped Sol
  {
    mint: new PublicKey("So11111111111111111111111111111111111111112"),
    decimals: 9,
  },
  // USDC
  {
    mint: new PublicKey("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"),
    decimals: 6,
  },
  // USDT
  {
    mint: new PublicKey("Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB"),
    decimals: 6,
  },
];

// In seconds
const DEADLINES = [3, 6, 11];

type ComparisonResult = {
  requestTime: Date;
  responseTime: Date;
  deadline: number;
  inputTokenMint: PublicKey;
  outputTokenMint: PublicKey;
  inputAmount: number;
  quoteAmount: bigint;
  referenceId?: string;
};

async function getQuote(
  url: string,
  chainId: string,
  publicKey: PublicKey,
  inputToken: Token,
  outputToken: Token,
): Promise<ComparisonResult[]> {
  const client = new Client({
    baseUrl: url,
  });

  const result: ComparisonResult[] = [];
  let amount = 10000;
  if (
    inputToken.mint.equals(
      new PublicKey("So11111111111111111111111111111111111111112"),
    )
  ) {
    amount /= 100;
  }
  const jobs = DEADLINES.map(async (deadline) => {
    const amountConverted = amount * 10 ** inputToken.decimals + deadline;
    try {
      const requestTime = new Date();
      const response = await client.getQuote({
        chainId,
        inputTokenMint: inputToken.mint,
        outputTokenMint: outputToken.mint,
        specifiedTokenAmount: {
          amount: amountConverted,
          side: "input",
        },
        userWallet: publicKey,
        minimumLifetime: deadline,
      });
      const responseTime = new Date();
      result.push({
        requestTime,
        responseTime,
        deadline,
        inputTokenMint: inputToken.mint,
        outputTokenMint: outputToken.mint,
        inputAmount: amountConverted,
        quoteAmount: response.outputToken.amount,
        referenceId: response.referenceId,
      });
    } catch (error) {
      console.error("Error getting quote:", error);
    }
  });
  await Promise.all(jobs);

  // compare results
  const sortedResult = _.sortBy(result, ["deadline"]);
  for (let i = 0; i < sortedResult.length - 1; i++) {
    const current = sortedResult[i];
    const next = sortedResult[i + 1];
    const currentQuote = new Decimal(current.quoteAmount.toString());
    const nextQuote = new Decimal(next.quoteAmount.toString());
    const diffInBps = currentQuote.sub(nextQuote).div(currentQuote).mul(10000);
    console.log(
      `Current Deadline: ${current.deadline} | Next Deadline: ${
        next.deadline
      } | Input Amount: ${amount} | Diff: ${diffInBps.toString()} bps`,
    );
  }

  return result;
}

async function run() {
  const { serverUrl } = argv;
  const publicKey = new PublicKey(argv["public-key"]);

  console.log("--------------------------------------------------");
  console.log("Starting dynamic deadlinetest with the following parameters:");
  console.log(`- Public key: ${publicKey.toBase58()}`);
  console.log(`- Server URL: ${serverUrl}`);
  console.log("--------------------------------------------------");

  for (const inputToken of TOKENS) {
    for (const outputToken of TOKENS) {
      if (inputToken.mint.equals(outputToken.mint)) {
        continue;
      }
      console.log(
        `Testing ${inputToken.mint.toBase58()} -> ${outputToken.mint.toBase58()}`,
      );
      const results = await getQuote(
        serverUrl,
        argv["chain-id"],
        publicKey,
        inputToken,
        outputToken,
      );
      console.log(
        `Results for ${inputToken.mint.toBase58()} -> ${outputToken.mint.toBase58()}:`,
      );
      console.log("--------------------------------------------------");
      console.log(
        "Deadline (s) | Request Time (s) | Response Time (s) | Input Amount | Quote Amount | Reference Id",
      );
      console.log("--------------------------------------------------");
      results.forEach((result) => {
        console.log(
          `${result.deadline} | ${result.requestTime} | ${
            result.responseTime
          } | ${result.inputAmount} | ${result.quoteAmount.toString()} | ${
            result.referenceId || "N/A"
          }`,
        );
      });
    }
  }
}

run();
