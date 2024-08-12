import * as anchor from "@coral-xyz/anchor";
import {
  Connection,
  PublicKey,
  Keypair,
  AddressLookupTableProgram,
} from "@solana/web3.js";

import { waitForNewBlock } from "./sleep";

export async function createLookupTableIx(
  authority: PublicKey,
  payer: PublicKey,
  slot: number
): Promise<[anchor.web3.TransactionInstruction, PublicKey]> {
  const [lookupTableInst, lookupTableAddress] =
    AddressLookupTableProgram.createLookupTable({
      authority: authority,
      payer: payer,
      recentSlot: slot,
    });
  return [lookupTableInst, lookupTableAddress];
}

export async function extendLookupTableIx(
  lookupTable: PublicKey,
  authority: PublicKey,
  payer: PublicKey,
  addresses: PublicKey[]
): Promise<anchor.web3.TransactionInstruction> {
  const extendInstruction = AddressLookupTableProgram.extendLookupTable({
    payer: payer,
    authority: authority,
    lookupTable: lookupTable,
    addresses: addresses,
  });
  return extendInstruction;
}

export async function createAndPopulateLookupTable(
  c: Connection,
  accounts: Set<PublicKey>,
  authority: Keypair,
  payer: Keypair,
  lookupTable?: PublicKey
): Promise<PublicKey> {
  let slot = (await c.getSlot()) - 1;

  const transactionLookupTable = new anchor.web3.Transaction();

  let lookupTableAddress;

  if (!lookupTable) {
    const createLookupTableOutput = await createLookupTableIx(
      authority.publicKey,
      payer.publicKey,
      slot
    );
    const lookupTableInst = createLookupTableOutput[0];
    lookupTableAddress = createLookupTableOutput[1];
    transactionLookupTable.add(lookupTableInst);
  } else {
    lookupTableAddress = lookupTable;
  }

  const extendInstruction = await extendLookupTableIx(
    lookupTableAddress,
    authority.publicKey,
    payer.publicKey,
    Array.from(accounts)
  );
  transactionLookupTable.add(extendInstruction);
  let signatureLookupTable = await c
    .sendTransaction(transactionLookupTable, [authority, payer], {})
    .catch((err) => {
      console.error(err);
    });
  const latestBlockHashLookupTable = await c.getLatestBlockhash();
  await c.confirmTransaction({
    blockhash: latestBlockHashLookupTable.blockhash,
    lastValidBlockHeight: latestBlockHashLookupTable.lastValidBlockHeight,
    signature: signatureLookupTable,
  });

  // sleep to allow the lookup table to activate
  await waitForNewBlock(c, 1);

  return lookupTableAddress;
}
