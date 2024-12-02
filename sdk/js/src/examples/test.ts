import { Client, Opportunity } from "..";

async function main() {
  const opportunityHandler = async (opportunity: Opportunity) => {
    console.log("recevied opportunity");
    console.log(opportunity);
  };

  const client = new Client(
    {
      baseUrl: "https://pyth-express-relay-mainnet.asymmetric.re/",
    },
    undefined,
    opportunityHandler
  );
  await client.subscribeChains(["solana"]);
}

main();
