import yargs from "yargs";
import { hideBin } from "yargs/helpers";
import { privateKeyToAccount } from "viem/accounts";
import {
  Address,
  PrivateKeyAccount,
  encodeAbiParameters,
  encodeFunctionData,
  isHex,
} from "viem";
import { abi } from "./abi";
import * as fs from "fs";
import * as path from "path";

interface Opportunity {
  sellToken: string;
  buyToken: string;
  sellAmount: string;
  buyAmount: string;
}

interface Config {
  serverUrl: string;
  opportunityProvider: string;
  permit2: string;
  chainNetworkId: number;
  chainId: string;
}

function readConfig(): Config {
  const configPath = path.join("config.json");
  const data = fs.readFileSync(configPath, "utf8");
  return JSON.parse(data) as Config;
}

function readOpportunity(): Opportunity {
  const data = fs.readFileSync("opportunity.json", "utf8");
  JSON.parse(data) as Opportunity;
  return JSON.parse(data) as Opportunity;
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
          amount: opportunity.buyAmount,
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

function getCallData(
  account: PrivateKeyAccount,
  opportunity: Opportunity,
  nonce: number,
  deadline: number,
  signature: `0x${string}`
) {
  return encodeFunctionData({
    abi,
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
              amount: BigInt(opportunity.buyAmount),
            },
          ],
          owner: account.address,
        },
      },
      signature,
    ],
  });
}

async function submitOpportunity(account: PrivateKeyAccount) {
  const config = readConfig();
  const opportunity = readOpportunity();

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

  const params = {
    version: "v1",
    permission_key: permissionKey,
    chain_id: config.chainId,
    target_contract: config.opportunityProvider,
    target_calldata: getCallData(
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
        amount: opportunity.buyAmount,
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
  .help()
  .alias("help", "h")
  .parseSync();

async function run() {
  if (isHex(argv.privateKey)) {
    const account = privateKeyToAccount(argv.privateKey);
    console.log(`Using account: ${account.address}`);
    submitOpportunity(account);
  } else {
    throw new Error(`Invalid private key: ${argv.privateKey}`);
  }
}

run();
