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
  let { blockhash: latestBlockhash } = await connection.getLatestBlockhash(
    "confirmed"
  );

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
  let lastSlotChange = Date.now();

  connection.onSlotChange(async (_slotInfo) => {
    lastSlotChange = Date.now();
  });

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
  const updateLatestBlockhash = async () => {
    while (true) {
      latestBlockhash = (await connection.getLatestBlockhash("confirmed"))
        .blockhash;
      await new Promise((resolve) => setTimeout(resolve, 10000));
      if (Date.now() - lastSlotChange > 5000) {
        console.error(
          "Did not receive slot change in 5 seconds, because of rpc or websocket issues. Exiting"
        );
        process.exit(1);
      }
    }
  };
  const resubmitOpportunities = async () => {
    while (true) {
      submitExistingOpportunities().catch(console.error);
      // Wait for 1 minute before resubmitting
      await new Promise((resolve) => setTimeout(resolve, 60 * 1000));
    }
  };

  resubmitOpportunities().catch(console.error);
  updateLatestBlockhash().catch(console.error);
}

run();
