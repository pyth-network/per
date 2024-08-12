import {
  TransactionSignature,
  VersionedTransaction,
  Connection,
  Commitment,
  SendOptions,
} from "@solana/web3.js";

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
