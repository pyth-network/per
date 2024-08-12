import { PublicKey, Transaction, VersionedTransaction } from "@solana/web3.js";

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

export const getTxSize = (
  tx: Transaction,
  feePayer: PublicKey,
  verbose: boolean = false
): number => {
  const feePayerPk = [feePayer.toBase58()];

  const signers = new Set<string>(feePayerPk);
  const accounts = new Set<string>(feePayerPk);

  let ixNumber = -1;

  const ixsSize = tx.instructions.reduce((acc, ix) => {
    ix.keys.forEach(({ pubkey, isSigner }) => {
      const pk = pubkey.toBase58();
      if (isSigner) signers.add(pk);
      accounts.add(pk);
    });

    accounts.add(ix.programId.toBase58());

    const nIndexes = ix.keys.length;
    const opaqueData = ix.data.length;

    ixNumber += 1;

    if (verbose) {
      console.log("ix number", ixNumber);
      console.log("n accounts in ix: ", nIndexes);
      console.log("length of data in ix: ", opaqueData);
    } else {
      console.debug("ix number", ixNumber);
      console.debug("n accounts in ix: ", nIndexes);
      console.debug("length of data in ix: ", opaqueData);
    }
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

  if (verbose) {
    console.log("Size of header: ", sizeHeader);
    console.log("Size of blockhash: ", sizeBlockhash);
    console.log("Size of signatures: ", sizeSignatures);
    console.log("Size of accounts: ", sizeAccounts);
    console.log("Size of number of instructions: ", sizeNInstructions);
    console.log("Size of ixs: ", ixsSize);
  } else {
    console.debug("Size of header: ", sizeHeader);
    console.debug("Size of blockhash: ", sizeBlockhash);
    console.debug("Size of signatures: ", sizeSignatures);
    console.debug("Size of accounts: ", sizeAccounts);
    console.debug("Size of number of instructions: ", sizeNInstructions);
    console.debug("Size of ixs: ", ixsSize);
  }

  return (
    sizeSignatures +
    sizeHeader +
    sizeAccounts +
    sizeBlockhash +
    sizeNInstructions +
    ixsSize
  );
};

export const getVersionedTxSize = (
  tx: VersionedTransaction,
  feePayer: PublicKey,
  verbose: boolean = false
): number => {
  const feePayerPk = [feePayer.toBase58()];

  const accounts = new Set<string>(feePayerPk);

  tx.message.staticAccountKeys.forEach((key) => {
    accounts.add(key.toBase58());
  });

  const lookupSize = tx.message.addressTableLookups.reduce((acc, lookup) => {
    const nWritable = lookup.writableIndexes.length;
    const nReadable = lookup.readonlyIndexes.length;

    return (
      acc +
      32 + // LUT address size
      compactArraySize(nWritable, 1) +
      compactArraySize(nReadable, 1)
    );
  }, 0);

  let ixNumber = -1;

  const ixsSize = tx.message.compiledInstructions.reduce((acc, ix) => {
    const nIndexes = ix.accountKeyIndexes.length;
    const opaqueData = ix.data.length;

    ixNumber += 1;

    if (verbose) {
      console.log("ix number", ixNumber);
      console.log("n accounts in ix: ", nIndexes);
      console.log("length of data in ix: ", opaqueData);
    } else {
      console.debug("ix number", ixNumber);
      console.debug("n accounts in ix: ", nIndexes);
      console.debug("length of data in ix: ", opaqueData);
    }

    return (
      acc +
      1 + // PID index
      compactArraySize(nIndexes, 1) +
      compactArraySize(opaqueData, 1)
    );
  }, 0);

  let nSigners = tx.message.header.numRequiredSignatures;

  const sizeSignatures = compactArraySize(nSigners, 64);
  const sizeHeader = 3;
  const sizeAccounts = compactArraySize(accounts.size, 32);
  const sizeBlockhash = 32;
  const sizeNInstructions = compactHeader(
    tx.message.compiledInstructions.length
  );

  if (verbose) {
    console.log("Size of header: ", sizeHeader);
    console.log("Size of blockhash: ", sizeBlockhash);
    console.log("Size of signatures: ", sizeSignatures);
    console.log("Size of accounts: ", sizeAccounts);
    console.log("Size of number of instructions: ", sizeNInstructions);
    console.log("Size of ixs: ", ixsSize);
    console.log("Size of lookup: ", lookupSize);
  } else {
    console.debug("Size of header: ", sizeHeader);
    console.debug("Size of blockhash: ", sizeBlockhash);
    console.debug("Size of signatures: ", sizeSignatures);
    console.debug("Size of accounts: ", sizeAccounts);
    console.debug("Size of number of instructions: ", sizeNInstructions);
    console.debug("Size of ixs: ", ixsSize);
    console.debug("Size of lookup: ", lookupSize);
  }

  return (
    sizeSignatures +
    sizeHeader +
    sizeAccounts +
    sizeBlockhash +
    sizeNInstructions +
    ixsSize +
    lookupSize
  );
};
