import { PublicKey } from "@solana/web3.js";

// COMPACT ARRAY

const LOW_VALUE = 127; // 0x7f
const HIGH_VALUE = 16383; // 0x3fff

/**
 * Compact u16 array header size
 * @param n elements in the compact array
 * @returns size in bytes of array header
 */
const compactHeader = (n: number) =>
  n <= LOW_VALUE ? 1 : n <= HIGH_VALUE ? 2 : 3;

/**
 * Compact u16 array size
 * @param n elements in the compact array
 * @param size bytes per each element
 * @returns size in bytes of array
 */
const compactArraySize = (n: number, size: number) =>
  compactHeader(n) + n * size;

export const getTxSize = (tx: Transaction, feePayer: PublicKey): number => {
  const feePayerPk = [feePayer.toBase58()];

  const signers = new Set<string>(feePayerPk);
  const accounts = new Set<string>(feePayerPk);

  const ixsSize = tx.instructions.reduce((acc, ix) => {
    ix.keys.forEach(({ pubkey, isSigner }) => {
      const pk = pubkey.toBase58();
      if (isSigner) signers.add(pk);
      accounts.add(pk);
    });

    accounts.add(ix.programId.toBase58());

    const nIndexes = ix.keys.length;
    const opaqueData = ix.data.length;

    console.log("n accounts in ix: ", nIndexes);
    console.log("length of data in ix: ", opaqueData);
    console.log("");
    return (
      acc +
      1 + // PID index
      compactArraySize(nIndexes, 1) +
      compactArraySize(opaqueData, 1)
    );
  }, 0);

  const sizeSignatures = compactArraySize(signers.size, 64);
  const sizeHeader = 3;
  const sizeAccounts = compactArraySize(accounts.size, 32);
  const sizeBlockhash = 32;
  const sizeNInstructions = compactHeader(tx.instructions.length);

  console.log("Size of signatures: ", sizeSignatures);
  console.log("Size of accounts: ", sizeAccounts);
  console.log("Size of number of instructions: ", sizeNInstructions);
  console.log("Size of ixs: ", ixsSize);

  return (
    sizeSignatures +
    sizeHeader +
    sizeAccounts +
    sizeBlockhash +
    sizeNInstructions +
    ixsSize
  );
};
