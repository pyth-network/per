import * as anchor from "@coral-xyz/anchor";
import {
  TransactionSignature,
  TransactionMessage,
  VersionedTransaction,
  Connection,
  Commitment,
  SendOptions,
  PublicKey,
  Keypair,
} from "@solana/web3.js";

import { getTxSize, getVersionedTxSize } from "./txSize";
import { createAndPopulateLookupTable } from "./lookupTable";

export async function sendAndConfirmVersionedTransaction(
  c: Connection,
  tx: VersionedTransaction,
  commitment: Commitment = "confirmed",
  sendTransactionOptions: SendOptions = { preflightCommitment: "processed" }
): Promise<TransactionSignature> {
  const defaultOptions: SendOptions = { skipPreflight: true };
  const txId = await c.sendTransaction(tx, {
    ...defaultOptions,
    ...sendTransactionOptions,
  });

  const latestBlockHash = await c.getLatestBlockhash("finalized");

  const t = await c.confirmTransaction(
    {
      blockhash: latestBlockHash.blockhash,
      lastValidBlockHeight: latestBlockHash.lastValidBlockHeight,
      signature: txId,
    },
    commitment
  );

  if (t.value && t.value.err) {
    const txDetails = await c.getTransaction(txId, {
      maxSupportedTransactionVersion: 0,
      commitment: "confirmed",
    });
    if (txDetails) {
      throw {
        err: txDetails.meta?.err,
        logs: txDetails.meta?.logMessages || [],
      };
    }
    throw t.value.err;
  }

  return txId;
}

export async function createAndSubmitTransaction(
  c: Connection,
  ixs: anchor.web3.TransactionInstruction[],
  lookupAccounts: PublicKey[],
  lookupPayer: Keypair,
  payer: PublicKey,
  signers: Keypair[],
  verbose: boolean = false
): Promise<[TransactionSignature, number]> {
  let transaction = new anchor.web3.Transaction();

  ixs.forEach((ix) => {
    transaction.add(ix);
  });
  let txSize = getTxSize(transaction, payer, verbose);
  console.log("Legacy transaction size: ", txSize);

  const latestBlockHash = await c.getLatestBlockhash();
  const message = new TransactionMessage({
    payerKey: payer,
    recentBlockhash: latestBlockHash.blockhash,
    instructions: transaction.instructions,
  });

  // do lookup tables stuff
  const lookupTableKey = await createAndPopulateLookupTable(
    c,
    new Set(lookupAccounts),
    lookupPayer,
    lookupPayer
  );
  const lookupTable = (await c.getAddressLookupTable(lookupTableKey)).value;

  const messageV0 = message.compileToV0Message([lookupTable]);

  let txFee = (await c.getFeeForMessage(messageV0)).value;

  const transactionV0 = new VersionedTransaction(messageV0);
  transactionV0.sign(signers);

  let txSizeV0 = getVersionedTxSize(transactionV0, payer, verbose);
  console.log("V0 transaction size: ", txSizeV0);

  let txResponse = await sendAndConfirmVersionedTransaction(c, transactionV0);

  return [txResponse, txFee];
}
