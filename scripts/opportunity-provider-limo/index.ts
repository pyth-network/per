import yargs from "yargs";
import { hideBin } from "yargs/helpers";
import * as fs from "fs";
import { Connection, Keypair, PublicKey } from "@solana/web3.js";
import * as anchor from "@coral-xyz/anchor";
import * as limo from "@kamino-finance/limo-sdk";
import { Decimal } from "decimal.js";
import { getMintDecimals } from "@kamino-finance/limo-sdk/dist/utils";

interface Token {
  mint: PublicKey;
  symbol: string;
}

interface OpportunityPair {
  token1: Token;
  token2: Token;
  randomizeSides: boolean;
  minAmountNotional: number;
  maxAmountNotional: number;
}

function readFile<T>(path: string): T {
  const data = fs.readFileSync(path, "utf8");
  return JSON.parse(data) as T;
}

function loadOpportunities(path: string): OpportunityPair[] {
  const opportunityPairs = readFile<OpportunityPair[]>(path);
  return opportunityPairs.map((opportunity) => {
    return {
      token1: {
        mint: new PublicKey(opportunity.token1.mint),
        symbol: opportunity.token1.symbol,
      },
      token2: {
        mint: new PublicKey(opportunity.token2.mint),
        symbol: opportunity.token2.symbol,
      },
      randomizeSides: opportunity.randomizeSides,
      minAmountNotional: opportunity.minAmountNotional,
      maxAmountNotional: opportunity.maxAmountNotional,
    };
  });
}

const decimals: Record<string, number> = {};
const prices: Record<string, number> = {};

async function getDecimals(
  connection: Connection,
  token: Token
): Promise<number> {
  const index = token.mint.toBase58();
  if (decimals[index] === undefined) {
    decimals[index] = await getMintDecimals(connection, token.mint);
  }

  return decimals[index];
}

async function getPrice(connection: Connection, token: Token): Promise<number> {
  const index = token.symbol;
  if (prices[index] === undefined) {
    const url = `https://api.binance.com/api/v3/ticker/price?symbol=${token.symbol}USDT`;
    const response = await fetch(url);
    const data = await response.json();
    const price = parseFloat(data.price);
    if (isNaN(price)) {
      throw new Error(`Invalid price: ${data.price}`);
    }
    prices[index] = price;
  }

  const mintDecimals = await getDecimals(connection, token);
  return prices[index] / Math.pow(10, mintDecimals);
}

async function createOpportunities(
  skExecutor: Keypair,
  connection: Connection,
  globalConfig: PublicKey,
  opportunitiesPath: string,
  count: number,
  markup: number
) {
  const opportunities = loadOpportunities(opportunitiesPath);
  for (const opportunity of opportunities) {
    for (let j = 0; j < count; j++) {
      let inputToken = opportunity.token1;
      let outputToken = opportunity.token2;
      if (opportunity.randomizeSides) {
        if (Math.random() > 0.5) {
          inputToken = opportunity.token2;
          outputToken = opportunity.token1;
        }
      }

      const priceInput = await getPrice(connection, inputToken);
      const priceOutput = await getPrice(connection, outputToken);

      if (opportunity.maxAmountNotional < opportunity.minAmountNotional) {
        throw new Error(
          `maxAmountNotional ${opportunity.maxAmountNotional} is less than minAmountNotional ${opportunity.minAmountNotional}`
        );
      }

      const notional =
        Math.random() *
          (opportunity.maxAmountNotional - opportunity.minAmountNotional) +
        opportunity.minAmountNotional;

      const amountInput = (notional * (1 + markup / 10_000)) / priceInput;
      const amountOutput = notional / priceOutput;

      console.log("Creating opportunity:");
      const decimalsInput = await getDecimals(connection, inputToken);
      const decimalsOutput = await getDecimals(connection, outputToken);
      console.log(
        `Input: ${inputToken.symbol}, ${
          amountInput / Math.pow(10, decimalsInput)
        }`
      );
      console.log(
        `Output: ${outputToken.symbol}, ${
          amountOutput / Math.pow(10, decimalsOutput)
        }`
      );

      const limoClient = new limo.LimoClient(
        new Connection(argv.endpointSvm),
        globalConfig
      );

      const signature = (
        await limoClient.createOrderGeneric(
          skExecutor,
          inputToken.mint,
          outputToken.mint,
          new Decimal(amountInput.toFixed()),
          new Decimal(amountOutput.toFixed())
        )
      )[0];
      console.log(`Created opportunity: ${signature}`);
    }
  }
}

const argv = yargs(hideBin(process.argv))
  .option("sk-payer", {
    description:
      "Secret key of address to submit transactions with. In 64-byte base58 format",
    type: "string",
    demandOption: true,
  })
  .option("global-config", {
    description: "Global config address",
    type: "string",
    demandOption: true,
  })
  .option("endpoint-svm", {
    description: "SVM RPC endpoint",
    type: "string",
    demandOption: true,
  })
  .option("opportunities", {
    description: "Path to opportunities file",
    type: "string",
    default: "opportunities.json",
  })
  .option("count", {
    description: "Number of opportunities to create",
    type: "number",
    default: 1,
  })
  .option("markup", {
    description:
      "Markup of the sold-off assets relative to the purchased assets, in basis points. e.g.: 100 = 1%",
    type: "number",
    default: 100,
  })
  .option("interval", {
    description: "Interval in seconds to wait between creating opportunities",
    type: "number",
    demandOption: false,
  })
  .option("verbose", {
    description: "Print logs",
    type: "boolean",
    default: false,
  })
  .help()
  .alias("help", "h")
  .parseSync();

async function run() {
  const skExecutor = Keypair.fromSecretKey(
    anchor.utils.bytes.bs58.decode(argv["sk-payer"])
  );
  console.log(`Using payer/creator: ${skExecutor.publicKey.toBase58()}`);

  const globalConfig = new PublicKey(argv.globalConfig);
  console.log(`Using global config: ${globalConfig.toBase58()}`);

  const connection = new Connection(argv.endpointSvm);
  const interval = argv.interval;

  while (true) {
    await createOpportunities(
      skExecutor,
      connection,
      globalConfig,
      argv.opportunities,
      argv.count,
      argv.markup
    );
    if (interval === undefined) {
      break;
    } else {
      await new Promise((resolve) => setTimeout(resolve, interval * 1000));
    }
  }
}

run();
