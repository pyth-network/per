import yargs from "yargs";
import { hideBin } from "yargs/helpers";
import { privateKeyToAccount } from "viem/accounts";
import {
  Address,
  PrivateKeyAccount,
  createPublicClient,
  encodeAbiParameters,
  encodeFunctionData,
  isHex,
  http,
} from "viem";
import { abi as providerAbi } from "./abi/provider";
import { abi as erc20Abi } from "./abi/erc20";
import * as fs from "fs";

interface Token {
  address: Address;
  symbol: string;
}

interface Opportunity {
  sellToken: Token;
  buyToken: Token;
  sellAmount: number;
  buyAmount?: number;
}

interface Config {
  serverUrl: string;
  opportunityProvider: string;
  permit2: string;
  chainNetworkId: number;
  chainId: string;
  rpcUrl: string;
}

const decimals: Record<Address, number> = {};
const prices: Record<string, number> = {};

async function getDecimals(config: Config, token: Token): Promise<number> {
  const index = token.address;
  if (decimals[index]) {
    return decimals[index];
  }

  const client = createPublicClient({
    transport: http(config.rpcUrl),
  });
  decimals[index] = await client.readContract({
    address: token.address,
    abi: erc20Abi,
    functionName: "decimals",
  });
  return decimals[index];
}

function readFile<T>(path: string): T {
  const data = fs.readFileSync(path, "utf8");
  return JSON.parse(data) as T;
}

async function getPrice(token: Token): Promise<number> {
  const index = token.symbol;
  if (prices[index]) {
    return prices[index];
  }

  if (token.symbol == "USDT") {
    prices[index] = 1;
    return 1;
  }

  const url = `https://api.binance.com/api/v3/ticker/price?symbol=${token.symbol}USDT`;
  const response = await fetch(url);
  const data = await response.json();
  const price = parseFloat(data.price);
  if (isNaN(price)) {
    throw new Error(`Invalid price: ${data.price}`);
  }

  prices[index] = price;
  return price;
}

async function getBuyAmount(
  config: Config,
  opportunity: Opportunity,
  threshold: number = 0.9,
): Promise<number> {
  if (opportunity.buyAmount) {
    return opportunity.buyAmount;
  }

  const sellUsdAmount = await getPrice(opportunity.sellToken);
  const buyUsdAmount = await getPrice(opportunity.buyToken);

  const buyAmount =
    ((opportunity.sellAmount * sellUsdAmount) / buyUsdAmount) * threshold;
  return buyAmount;
}

async function getDecimalParsed(
  config: Config,
  token: Token,
  amount: number,
): Promise<bigint> {
  const decimals = await getDecimals(config, token);
  return BigInt(Math.floor(amount * 10 ** decimals));
}

async function signOpportunity(
  account: PrivateKeyAccount,
  config: Config,
  opportunity: Opportunity,
  nonce: number,
  deadline: number,
) {
  const types = {
    PermitBatchWitnessTransferFrom: [
      { name: "permitted", type: "TokenPermissions[]" },
      { name: "spender", type: "address" },
      { name: "nonce", type: "uint256" },
      { name: "deadline", type: "uint256" },
      { name: "witness", type: "OpportunityProviderWitness" },
    ],
    OpportunityProviderWitness: [
      { name: "buyTokens", type: "TokenAmount[]" },
      { name: "owner", type: "address" },
    ],
    TokenAmount: [
      { name: "token", type: "address" },
      { name: "amount", type: "uint256" },
    ],
    TokenPermissions: [
      { name: "token", type: "address" },
      { name: "amount", type: "uint256" },
    ],
  };

  const buyAmount = await getBuyAmount(config, opportunity);
  const message = {
    permitted: [
      {
        token: opportunity.sellToken.address,
        amount: await getDecimalParsed(
          config,
          opportunity.sellToken,
          opportunity.sellAmount,
        ),
      },
    ],
    spender: config.opportunityProvider,
    nonce,
    deadline,
    witness: {
      buyTokens: [
        {
          token: opportunity.buyToken.address,
          amount: await getDecimalParsed(
            config,
            opportunity.buyToken,
            buyAmount,
          ),
        },
      ],
      owner: account.address,
    },
  };

  return account.signTypedData({
    domain: {
      name: "Permit2",
      verifyingContract: config.permit2 as Address,
      chainId: config.chainNetworkId,
    },
    types,
    primaryType: "PermitBatchWitnessTransferFrom",
    message,
  });
}

async function getCallData(
  config: Config,
  account: PrivateKeyAccount,
  opportunity: Opportunity,
  nonce: number,
  deadline: number,
  signature: `0x${string}`,
) {
  const buyAmount = await getBuyAmount(config, opportunity);
  return encodeFunctionData({
    abi: providerAbi,
    functionName: "execute",
    args: [
      {
        permit: {
          permitted: [
            {
              token: opportunity.sellToken.address,
              amount: await getDecimalParsed(
                config,
                opportunity.sellToken,
                opportunity.sellAmount,
              ),
            },
          ],
          nonce: BigInt(nonce),
          deadline: BigInt(deadline),
        },
        witness: {
          buyTokens: [
            {
              token: opportunity.buyToken.address,
              amount: await getDecimalParsed(
                config,
                opportunity.buyToken,
                buyAmount,
              ),
            },
          ],
          owner: account.address,
        },
      },
      signature,
    ],
  });
}

async function submitOpportunity(
  account: PrivateKeyAccount,
  config: Config,
  opportunity: Opportunity,
) {
  const nonce = Math.floor(Math.random() * 2 ** 50);
  const deadline = Math.floor(Date.now() / 1000) + 60 * 10;
  const signature = await signOpportunity(
    account,
    config,
    opportunity,
    nonce,
    deadline,
  );

  const permissionKey = encodeAbiParameters(
    [
      { type: "address", name: "admin" },
      { type: "bytes", name: "signature" },
    ],
    [account.address, signature],
  );

  const buyAmount = await getBuyAmount(config, opportunity);
  const params = {
    version: "v1",
    permission_key: permissionKey,
    chain_id: config.chainId,
    target_contract: config.opportunityProvider,
    target_calldata: await getCallData(
      config,
      account,
      opportunity,
      nonce,
      deadline,
      signature,
    ),
    target_call_value: "0",
    sell_tokens: [
      {
        token: opportunity.buyToken.address,
        amount: (
          await getDecimalParsed(config, opportunity.buyToken, buyAmount)
        ).toString(),
      },
    ],
    buy_tokens: [
      {
        token: opportunity.sellToken.address,
        amount: (
          await getDecimalParsed(
            config,
            opportunity.sellToken,
            opportunity.sellAmount,
          )
        ).toString(),
      },
    ],
  };

  console.log("Submitting opportunity...");
  const response = await fetch(`${config.serverUrl}/v1/opportunities`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
    },
    body: JSON.stringify(params),
  });
  console.log("Opportunity submitted", response.status);
  const data = await response.json();
  console.log(data);
}

async function submitOpportunities(
  account: PrivateKeyAccount,
  config: Config,
  opportunities: Opportunity[],
) {
  opportunities.forEach((opportunity) => {
    submitOpportunity(account, config, opportunity).catch((error) => {
      console.error("Error submitting opportunity", error);
    });
  });
}

async function loadAndSubmitOpportunities(
  account: PrivateKeyAccount,
  configPath: string,
  opportunityPath: string,
) {
  const config: Config = readFile(configPath);
  const opportunities: Opportunity[] = readFile(opportunityPath);
  submitOpportunities(account, config, opportunities);
}

// [min, max)
function sampleUniform(min: number, max: number) {
  if (min >= max) {
    throw new Error("Invalid range");
  }

  return Math.floor(Math.random() * (max - min) + min);
}

async function createAndSubmitRandomOpportunities(
  account: PrivateKeyAccount,
  configPath: string,
  tokensPath: string,
  count: number,
) {
  const config: Config = readFile(configPath);
  const tokens: Token[] = readFile(tokensPath);

  if (tokens.length < 2) {
    throw new Error("At least 2 tokens are required");
  }

  // Use simple for to make sure we are going to use the cached data
  const opportunities: Opportunity[] = [];
  for (let i = 0; i < count; i++) {
    const sellToken = tokens.filter((token) => token.symbol === "SOL")[0];
    let buyToken = tokens[sampleUniform(0, tokens.length)];
    while (sellToken === buyToken) {
      buyToken = tokens[sampleUniform(0, tokens.length)];
    }

    const sellAmount = sampleUniform(1, 10) / 10;
    const buyAmount = await getBuyAmount(
      config,
      {
        sellToken,
        buyToken,
        sellAmount,
      },
      sampleUniform(60, 80) / 100,
    );
    opportunities.push({
      sellToken,
      buyToken,
      sellAmount,
      buyAmount,
    });
  }

  submitOpportunities(account, config, opportunities);
}

const argv = yargs(hideBin(process.argv))
  .option("private-key", {
    description:
      "Private key to provide opportunity in hex format with 0x prefix. e.g: 0xdeadbeef...",
    type: "string",
    demandOption: true,
  })
  .option("config", {
    description: "Path to config file",
    type: "string",
    default: "config.json",
  })
  .option("opportunities", {
    description: "Path to opportunities file",
    type: "string",
    default: "opportunities.json",
  })
  .option("tokens", {
    description: "Path to tokens file",
    type: "string",
    default: "tokens.json",
  })
  .option("count", {
    description: "Number of opportunities to create",
    type: "number",
    default: 10,
  })
  .option("load-test", {
    description: "Create and submit random opportunities",
    type: "boolean",
    default: false,
  })
  .help()
  .alias("help", "h")
  .parseSync();

async function run() {
  if (!isHex(argv.privateKey)) {
    throw new Error(`Invalid private key: ${argv.privateKey}`);
  }

  const account = privateKeyToAccount(argv.privateKey);
  console.log(`Using account: ${account.address}`);

  if (argv.loadTest) {
    createAndSubmitRandomOpportunities(
      account,
      argv.config,
      argv.tokens,
      argv.count,
    );
  } else {
    loadAndSubmitOpportunities(account, argv.config, argv.opportunities);
  }
}

run();
