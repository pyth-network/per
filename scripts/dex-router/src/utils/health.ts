import { Connection } from "@solana/web3.js";

export async function checkRpcHealth(
  connection: Connection,
  threshold: number,
  interval: number
) {
  //eslint-disable-next-line no-constant-condition
  while (true) {
    try {
      const slot = await connection.getSlot("finalized");
      const blockTime = await connection.getBlockTime(slot);
      const timeNow = Date.now() / 1000;
      if (blockTime === null) {
        console.error(
          `Health Error (RPC endpoint): unable to poll block time for slot ${slot}`
        );
      } else if (blockTime < timeNow - threshold) {
        console.error(
          `Health Error (RPC endpoint): block time is stale by ${
            timeNow - blockTime
          } seconds`
        );
      }
    } catch (e) {
      console.error("Health Error (RPC endpoint), failure to fetch: ", e);
    }
    await new Promise((resolve) => setTimeout(resolve, interval * 1000));
  }
}

export async function checkExpressRelayHealth(
  endpoint: string,
  interval: number
) {
  const urlExpressRelayHealth = new URL("/live", endpoint);
  //eslint-disable-next-line no-constant-condition
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
    await new Promise((resolve) => setTimeout(resolve, interval * 1000));
  }
}
