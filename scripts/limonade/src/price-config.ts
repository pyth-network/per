import { HexString } from "@pythnetwork/hermes-client";
import { Connection, PublicKey } from "@solana/web3.js";
import yaml from "yaml";
import fs from "fs";
import {
  getMint,
  TOKEN_2022_PROGRAM_ID,
  TOKEN_PROGRAM_ID,
} from "@solana/spl-token";

const TOKEN_PROGRAMS = [TOKEN_PROGRAM_ID, TOKEN_2022_PROGRAM_ID];

export type PriceConfig = {
  alias: string;
  mint: PublicKey;
  pythFeedId: HexString;
  decimals: number;
};

export async function loadPriceConfig(
  path: string,
  connection: Connection,
): Promise<PriceConfig[]> {
  const priceConfigs = yaml.parse(fs.readFileSync(path, "utf8"));

  for (const priceConfig of priceConfigs) {
    priceConfig.decimals = await getMintDecimals(
      connection,
      new PublicKey(priceConfig.mint),
    );
  }

  return priceConfigs.map(
    (priceConfig: {
      alias: string;
      mint: string;
      id: string;
      decimals: number;
    }) => ({
      alias: priceConfig.alias,
      mint: new PublicKey(priceConfig.mint),
      pythFeedId: priceConfig.id,
      decimals: priceConfig.decimals,
    }),
  );
}

async function getMintDecimals(
  connection: Connection,
  mint: PublicKey,
): Promise<number> {
  for (const programId of TOKEN_PROGRAMS) {
    try {
      const info = await getMint(connection, mint, undefined, programId);
      return info.decimals;
    } catch {
      continue;
    }
  }
  throw new Error(`Mint decimals not found for ${mint.toBase58()}`);
}
