import { HexString } from "@pythnetwork/hermes-client";
import { PublicKey } from "@solana/web3.js";
import yaml from "yaml";
import fs from "fs";

export type PriceConfig = {
  alias: string;
  mint: PublicKey;
  pythFeedId: HexString;
};

export function readPriceConfigFile(path: string): PriceConfig[] {
  try {
    const priceConfigs = yaml.parse(fs.readFileSync(path, "utf8"));
    return priceConfigs.map((priceConfig: any) => ({
      alias: priceConfig.alias,
      mint: new PublicKey(priceConfig.mint),
      pythFeedId: priceConfig.id,
    }));
  } catch (error) {
    console.error(`Error reading price config file ${path}: ${error}`);
    throw error;
  }
}
