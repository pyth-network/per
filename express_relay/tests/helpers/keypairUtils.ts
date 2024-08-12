// https://gist.github.com/0xdeepmehta/8be9713fc14587a4576c9c4d96ae65ff

import * as anchor from "@coral-xyz/anchor";
import * as fs from "fs/promises";
import * as path from "path";

export const writeKeypairToFile = async (
  sk: Uint8Array,
  filePath: string
): Promise<void> => {
  try {
    await fs.writeFile(filePath, JSON.stringify(Array.from(sk)));
    console.debug(`Keypair written to file: ${filePath}`);
  } catch (error) {
    console.error(`Error writing keypair to file: ${(error as Error).message}`);
  }
};

export const readKeypairFromFile = async (
  filePath: string
): Promise<anchor.web3.Keypair | undefined> => {
  try {
    const raw = await fs.readFile(filePath);
    const formattedData = JSON.parse(raw.toString());

    const keypair = anchor.web3.Keypair.fromSecretKey(
      Uint8Array.from(formattedData)
    );
    console.debug(
      `Read`,
      keypair.publicKey.toString(),
      `from file: ${filePath}`
    );
    return keypair;
  } catch (error) {
    console.error(
      `Error reading keypair from file: ${(error as Error).message}`
    );
  }
};
