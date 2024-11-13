import {
  Connection,
  GetProgramAccountsFilter,
  PublicKey,
} from "@solana/web3.js";
import { limoId, Order } from "@kamino-finance/limo-sdk";
import bs58 from "bs58";
import { hideBin } from "yargs/helpers";
import yargs from "yargs";
import {
  ChainType,
  Client,
  OpportunityCreate,
} from "@pythnetwork/express-relay-js";
import { getPdaAuthority } from "@kamino-finance/limo-sdk/dist/utils";

const lastChange: Record<string, number> = {};

const argv = yargs(hideBin(process.argv))
  .option("endpoint", {
    description:
      "Express relay endpoint. e.g: https://per-staging.dourolabs.app/",
    type: "string",
    default: "https://pyth-express-relay-mainnet.asymmetric.re/",
  })
  .option("rpc-endpoint", {
    description:
      "Solana rpc endpoint to use for subscribing to program account changes.",
    type: "string",
    default: "http://127.0.0.1:8899",
  })
  .option("global-config", {
    description:
      "Pubkey of the global config to filter for in limo program orders",
    type: "string",
    demandOption: true,
  })
  .option("chain-id", {
    description: "Chain id to publish these orders for as opportunity",
    type: "string",
    demandOption: true,
  })
  .option("api-key", {
    description:
      "API key to authenticate with the express relay server for publishing opportunities.",
    type: "string",
    demandOption: true,
  })
  .help()
  .alias("help", "h")
  .parseSync();

async function run() {
  const connection = new Connection(argv.rpcEndpoint);

  const globalConfig = new PublicKey(argv.globalConfig);
  let filters: GetProgramAccountsFilter[] = [
    {
      memcmp: {
        bytes: globalConfig.toBase58(),
        offset: 8,
      },
    },
    {
      memcmp: {
        bytes: bs58.encode(Order.discriminator),
        offset: 0,
      },
    },
  ];

  console.log("Listening for program account changes");
  const client = new Client({ baseUrl: argv.endpoint, apiKey: argv.apiKey });

  const submitExistingOpportunities = async () => {
    const response = await connection.getProgramAccounts(limoId, {
      commitment: "confirmed",
      filters,
      withContext: true,
    });

    const payloads: OpportunityCreate[] = response.value
      .filter(
        (account) =>
          lastChange[account.pubkey.toBase58()] === undefined ||
          lastChange[account.pubkey.toBase58()] < Date.now() - 60 * 1000
      )
      .map((account) => ({
        program: "limo" as const,
        chainId: argv.chainId,
        slot: response.context.slot,
        order: {
          state: Order.decode(account.account.data),
          address: account.pubkey,
        },
      }))
      .filter(
        (opportunityCreate) =>
          opportunityCreate.order.state.remainingInputAmount.toNumber() !== 0
      );

    console.log("Resubmitting opportunities", payloads.length);
    for (const payload of payloads) {
      try {
        await client.submitOpportunity(payload);
      } catch (e) {
        console.error(e);
      }
    }
  };

  connection.onProgramAccountChange(
    limoId,
    async (info, context) => {
      const order = Order.decode(info.accountInfo.data);
      if (order.remainingInputAmount.toNumber() === 0) {
        const router = getPdaAuthority(limoId, globalConfig);

        try {
          await client.removeOpportunity({
            chainType: ChainType.SVM,
            program: "limo",
            chainId: argv.chainId,
            permissionAccount: info.accountId,
            router,
          });
        } catch (e) {
          console.error("Failed to remove opportunity", e);
        }
        return;
      }
      console.log(
        "Fetched order with address:",
        info.accountId.toBase58(),
        "slot:",
        context.slot
      );

      const payload: OpportunityCreate = {
        program: "limo",
        chainId: argv.chainId,
        slot: context.slot,
        order: { state: order, address: info.accountId },
      };

      try {
        await client.submitOpportunity(payload);
        lastChange[info.accountId.toBase58()] = Date.now();
      } catch (e) {
        console.error("Failed to submit opportunity", e);
      }
    },
    {
      commitment: "processed",
      filters,
    }
  );
  const resubmitOpportunities = async () => {
    while (true) {
      submitExistingOpportunities().catch(console.error);
      // Wait for 1 minute before resubmitting
      await new Promise((resolve) => setTimeout(resolve, 60 * 1000));
    }
  };

  const RPC_HEALTH_CHECK_SECONDS_THRESHOLD = 300;
  const checkRpcHealth = async () => {
    while (true) {
      try {
        const slot = await connection.getSlot("finalized");
        const blockTime = await connection.getBlockTime(slot);
        const timeNow = Date.now() / 1000;
        if (blockTime === null) {
          console.error(
            `Health Error (RPC endpoint): unable to poll block time for slot ${slot}`
          );
        } else if (blockTime < timeNow - RPC_HEALTH_CHECK_SECONDS_THRESHOLD) {
          console.error(
            `Health Error (RPC endpoint): block time is stale by ${
              timeNow - blockTime
            } seconds`
          );
        }
      } catch (e) {
        console.error("Health Error (RPC endpoint), failure to fetch: ", e);
      }
      // Wait for 10 seconds before rechecking
      await new Promise((resolve) => setTimeout(resolve, 10 * 1000));
    }
  };

  const urlExpressRelayHealth = new URL("/live", argv.endpoint);
  const checkExpressRelayHealth = async () => {
    while (true) {
      try {
        const responseHealth = await fetch(urlExpressRelayHealth);
        if (responseHealth.status !== 200) {
          console.error(
            "Health Error (Express Relay endpoint): ",
            responseHealth
          );
        }
      } catch (e) {
        console.error(
          "Health Error (Express Relay endpoint), failure to fetch: ",
          e
        );
      }
      // Wait for 10 seconds before rechecking
      await new Promise((resolve) => setTimeout(resolve, 10 * 1000));
    }
  };

  resubmitOpportunities().catch(console.error);
  checkRpcHealth().catch(console.error);
  checkExpressRelayHealth().catch(console.error);
}

run();
