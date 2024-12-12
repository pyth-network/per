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
import { HermesClient, PriceUpdate } from "@pythnetwork/hermes-client";
import { PriceConfig, loadPriceConfig } from "./price-config";

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
    default: 100,
  })
  .option("solana-websocket-timeout", {
    description: "Solana websocket timeout (milliseconds)",
    type: "number",
    default: 30 * 1000,
  })
  .option("hermes-streaming-timeout", {
    description: "Hermes streaming timeout (milliseconds)",
    type: "number",
    default: 5 * 1000,
  })
  .option("price-config", {
    description:
      "Path to the price config file, if not provided, we will not get price info from Hermes",
    type: "string",
  })
  .option("hermes-endpoint", {
    description: "Hermes endpoint to use for fetching price updates",
    type: "string",
    default: "https://hermes.pyth.network/",
  })
  .option("active-opportunity-price-band", {
    description:
      "Number of basis points an opportunity's implied price needs to be within the price from Hermes for the opportunity to be considered active",
    type: "number",
    default: 500,
  })
  .option("price-staleness-threshold", {
    description:
      "Threshold of price staleness (milliseconds). If price for quote or base is stale, we will submit all existing opportunities, even those that are off-market",
    type: "number",
    default: 10 * 1000,
  })
  .option("resubmission-interval", {
    description:
      "Interval of resubmission (milliseconds). We will resubmit all existing active opportunities every interval",
    type: "number",
    default: 50 * 1000,
  })
  .help()
  .alias("help", "h")
  .parseSync();

async function run() {
  const connection = new Connection(argv.rpcEndpoint);
  const priceStore: Record<
    string,
    {
      price: string;
      exponent: number;
      mintDecimals: number;
      publishTime: number;
    }
  > = {};

  const globalConfig = new PublicKey(argv.globalConfig);
  const numberOfConcurrentSubmissions = argv.numberOfConcurrentSubmissions;
  let solanaConnectionTimeout: NodeJS.Timeout | undefined;
  let hermesConnectionTimeout: NodeJS.Timeout | undefined;

  const priceConfigs: PriceConfig[] = argv.priceConfig
    ? await loadPriceConfig(argv.priceConfig, connection)
    : [];

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

  const handleSubmitError = (e: unknown) => {
    if (
      !(
        e instanceof ClientError &&
        e.message.includes("Same opportunity is submitted recently")
      )
    ) {
      console.error("Failed to submit opportunity", e);
    }
  };
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
      )
      .filter((opportunityCreate) =>
        isWithinMarketPriceBand(opportunityCreate.order)
      );

    console.log("Resubmitting opportunities", payloads.length);
    for (let i = 0; i < payloads.length; i += numberOfConcurrentSubmissions) {
      const batch = payloads.slice(i, i + numberOfConcurrentSubmissions);
      await Promise.all(
        batch.map(async (payload) => {
          try {
            await client.submitOpportunity(payload);
          } catch (e) {
            handleSubmitError(e);
          }
        })
      );
    }
  };

  const isWithinMarketPriceBand = (order: {
    state: Order;
    address: PublicKey;
  }) => {
    const priceInputMint = priceStore[order.state.inputMint.toString()];
    const priceOutputMint = priceStore[order.state.outputMint.toString()];

    const now = Date.now();
    if (
      !priceInputMint ||
      !priceOutputMint ||
      now - priceInputMint.publishTime * 1000 > argv.priceStalenessThreshold ||
      now - priceOutputMint.publishTime * 1000 > argv.priceStalenessThreshold
    ) {
      // If we don't have price info, we will not consider it off-market
      return true;
    } else {
      const inputAmount = order.state.remainingInputAmount;
      const outputAmount = order.state.expectedOutputAmount.sub(
        order.state.filledOutputAmount
      );

      const ratio =
        (outputAmount.toNumber() / inputAmount.toNumber()) *
        (Number(priceOutputMint.price) / Number(priceInputMint.price)) *
        10 **
          (priceOutputMint.exponent -
            priceInputMint.exponent +
            priceInputMint.mintDecimals -
            priceOutputMint.mintDecimals);

      if (ratio < 1 + argv.activeOpportunityPriceBand / 10000) {
        return true;
      }

      return false;
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
          handleSubmitError(e);
        }
      }
      handleUpdate().catch(console.error);
    },
    {
      commitment: "processed",
      filters,
    }
  );

  connection.onSlotChange(() => {
    if (solanaConnectionTimeout !== undefined) {
      clearTimeout(solanaConnectionTimeout);
    }

    solanaConnectionTimeout = setTimeout(() => {
      throw new Error("Solana websocket timeout");
    }, argv.solanaWebsocketTimeout);
  });

  if (priceConfigs.length > 0) {
    const hermesClient = new HermesClient(argv.hermesEndpoint, {});

    const eventSource = await hermesClient.getPriceUpdatesStream(
      priceConfigs.map((priceConfig) => priceConfig.pythFeedId),
      {
        encoding: "hex",
        parsed: true,
        ignoreInvalidPriceIds: true,
        allowUnordered: true,
      }
    );

    eventSource.onerror = (event: Event) => {
      console.error("Hermes streaming error", event);
    };

    // Await for the first message before continuing
    await new Promise<void>((resolve, reject) => {
      setTimeout(() => {
        reject(new Error("Hermes streaming timeout"));
      }, argv.hermesStreamingTimeout);

      eventSource.onmessage = (event: MessageEvent<string>) => {
        resolve();

        const data: PriceUpdate = JSON.parse(event.data);
        if (data.parsed) {
          for (const parsedUpdate of data.parsed) {
            const priceConfig = priceConfigs.find(
              (priceConfig) => priceConfig.pythFeedId === parsedUpdate.id
            );
            if (priceConfig) {
              const currentPrice = priceStore[priceConfig.mint.toString()];
              if (
                currentPrice === undefined ||
                parsedUpdate.price.publish_time > currentPrice.publishTime
              ) {
                priceStore[priceConfig.mint.toString()] = {
                  price: parsedUpdate.price.price,
                  exponent: parsedUpdate.price.expo,
                  mintDecimals: priceConfig.decimals,
                  publishTime: parsedUpdate.price.publish_time,
                };
              }
            }
          }
        }

        if (hermesConnectionTimeout !== undefined) {
          clearTimeout(hermesConnectionTimeout);
        }

        hermesConnectionTimeout = setTimeout(() => {
          throw new Error("Hermes streaming timeout");
        }, argv.hermesStreamingTimeout);
      };
    });
  }

  const resubmitOpportunities = async () => {
    //eslint-disable-next-line no-constant-condition
    while (true) {
      submitExistingOpportunities().catch(console.error);
      // Server expires opportunities after 2 minutes
      // We should resubmit them before server expire them to avoid creating a new row in the database
      await new Promise((resolve) =>
        setTimeout(resolve, argv.resubmissionInterval)
      );
    }
  };

  resubmitOpportunities().catch(console.error);
}

run();
