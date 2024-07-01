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

interface Opportunity {
  sellToken: string;
  buyToken: string;
  sellAmount: string;
  buyAmount?: string;
  sellSymbol?: string;
  buySymbol?: string;
}

interface Config {
  serverUrl: string;
  opportunityProvider: string;
  permit2: string;
  chainNetworkId: number;
  chainId: string;
  rpcUrl: string;
}

function readConfig(path: string): Config {
  const data = fs.readFileSync(path, "utf8");
  return JSON.parse(data) as Config;
}

function readOpportunity(path: string): Opportunity {
  const data = fs.readFileSync(path, "utf8");
  JSON.parse(data) as Opportunity;
  return JSON.parse(data) as Opportunity;
}

function getClinet(config: Config) {
  return createPublicClient({
    transport: http(config.rpcUrl),
  });
}

async function getPrice(symbol: string): Promise<number> {
  if (symbol == "USDT") {
    return 1;
  }

  const url = `https://api.binance.com/api/v3/ticker/price?symbol=${symbol}USDT`;
  const response = await fetch(url);
  const data = await response.json();
  const price = parseFloat(data.price);
  if (isNaN(price)) {
    throw new Error(`Invalid price: ${data.price}`);
  }

  return price;
}

let buyAmount: bigint | undefined;
async function getBuyAmount(
  config: Config,
  opportunity: Opportunity
): Promise<bigint> {
  if (buyAmount) {
    return buyAmount;
  }

  if (opportunity.buyAmount) {
    buyAmount = BigInt(opportunity.buyAmount);
    return buyAmount;
  }

  if (!opportunity.buySymbol) {
    throw new Error("Missing buySymbol");
  }

  if (!opportunity.sellSymbol) {
    throw new Error("Missing sellSymbol");
  }

  const sellUsdAmount = await getPrice(opportunity.sellSymbol);
  const buyUsdAmount = await getPrice(opportunity.buySymbol);

  const client = getClinet(config);
  const decimalsSellToken = await client.readContract({
    address: opportunity.sellToken as Address,
    abi: erc20Abi,
    functionName: "decimals",
  });
  const decimalsBuyToken = await client.readContract({
    address: opportunity.sellToken as Address,
    abi: erc20Abi,
    functionName: "decimals",
  });

  const multiplier =
    (sellUsdAmount /
      buyUsdAmount /
      10 ** (decimalsSellToken - decimalsBuyToken)) *
    0.9;
  buyAmount = BigInt(
    Math.floor(parseFloat(opportunity.sellAmount) * multiplier)
  );

  return buyAmount;
}

async function signOpportunity(
  account: PrivateKeyAccount,
  config: Config,
  opportunity: Opportunity,
  nonce: number,
  deadline: number
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

  const message = {
    permitted: [
      {
        token: opportunity.sellToken,
        amount: opportunity.sellAmount,
      },
    ],
    spender: config.opportunityProvider,
    nonce,
    deadline,
    witness: {
      buyTokens: [
        {
          token: opportunity.buyToken,
          amount: await getBuyAmount(config, opportunity),
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
  signature: `0x${string}`
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
              token: opportunity.sellToken as Address,
              amount: BigInt(opportunity.sellAmount),
            },
          ],
          nonce: BigInt(nonce),
          deadline: BigInt(deadline),
        },
        witness: {
          buyTokens: [
            {
              token: opportunity.buyToken as Address,
              amount: buyAmount,
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
  configPath: string,
  opportunityPath: string
) {
  const config = readConfig(configPath);
  const opportunity = readOpportunity(opportunityPath);

  const nonce = Math.floor(Math.random() * 2 ** 50);
  const deadline = Math.floor(Date.now() / 1000) + 60 * 60 * 24;
  const signature = await signOpportunity(
    account,
    config,
    opportunity,
    nonce,
    deadline
  );
  const permissionKey = encodeAbiParameters(
    [
      { type: "address", name: "admin" },
      { type: "bytes", name: "signature" },
    ],
    [account.address, signature]
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
      signature
    ),
    target_call_value: "0",
    sell_tokens: [
      {
        token: opportunity.buyToken,
        amount: buyAmount.toString(),
      },
    ],
    buy_tokens: [
      {
        token: opportunity.sellToken,
        amount: opportunity.sellAmount,
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
  .option("opportunity", {
    description: "Path to opportunity file",
    type: "string",
    default: "opportunity.json",
  })
  .help()
  .alias("help", "h")
  .parseSync();

async function run() {
  if (isHex(argv.privateKey)) {
    const account = privateKeyToAccount(argv.privateKey);
    console.log(`Using account: ${account.address}`);
    submitOpportunity(account, argv.config, argv.opportunity);
  } else {
    throw new Error(`Invalid private key: ${argv.privateKey}`);
  }
}

run();
