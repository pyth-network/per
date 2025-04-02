import {
  getAccountLenForMint,
  getMint,
  TOKEN_2022_PROGRAM_ID,
} from "@solana/spl-token";
import { Connection, PublicKey } from "@solana/web3.js";

async function main() {
  const connection = new Connection("https://api.mainnet-beta.solana.com");
  const mint = await getMint(
    connection,
    new PublicKey("DSXVmrBySfBcmdNDGQkk59hGwXhAKNjEwc4as8nfmysd"),
    undefined,
    TOKEN_2022_PROGRAM_ID,
  );
  const accountLen = getAccountLenForMint(mint);
  console.log(accountLen);
}

main();
