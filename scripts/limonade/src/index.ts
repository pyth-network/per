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
  ClientError,
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
  .option("number-of-concurrent-submissions", {
    description: "Number of concurrent submissions to the express relay server",
    type: "number",
    default: 10,
  })
  .help()
  .alias("help", "h")
  .parseSync();

async function run() {
  const connection = new Connection(argv.rpcEndpoint);

  const globalConfig = new PublicKey(argv.globalConfig);
  const numberOfConcurrentSubmissions = argv.numberOfConcurrentSubmissions;
  const filters: GetProgramAccountsFilter[] = [
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
    for (let i = 0; i < payloads.length; i += numberOfConcurrentSubmissions) {
      const batch = payloads.slice(i, i + numberOfConcurrentSubmissions);
      await Promise.all(
        batch.map(async (payload) => {
          try {
            await client.submitOpportunity(payload);
          } catch (e) {
            if (
              e instanceof ClientError &&
              e.message.includes("Same opportunity is submitted recently")
            ) {
              console.log(e); // We don't want to pollute stderr with this
            } else {
              console.error(e);
            }
          }
        })
      );
    }
  };

  connection.onProgramAccountChange(
    limoId,
    (info, context) => {
      async function handleUpdate() {
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
      }
      handleUpdate().catch(console.error);
    },
    {
      commitment: "processed",
      filters,
    }
  );
  const resubmitOpportunities = async () => {
    //eslint-disable-next-line no-constant-condition
    while (true) {
      submitExistingOpportunities().catch(console.error);
      // Server expires opportunities after 2 minutes
      // We should resubmit them before server expire them to avoid creating a new row in the database
      await new Promise((resolve) => setTimeout(resolve, 50 * 1000));
    }
  };

  resubmitOpportunities().catch(console.error);
}

run();
