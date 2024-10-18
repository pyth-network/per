import {
  AddressLookupTableProgram,
  Connection,
  Keypair,
  PublicKey,
  sendAndConfirmTransaction,
  Transaction,
  TransactionSignature,
} from "@solana/web3.js";
import yargs from "yargs";
import { hideBin } from "yargs/helpers";
import * as anchor from "@coral-xyz/anchor";
import { LOOKUP_TABLE_ADDRESS } from "../const";

async function makeLookupTable(
  connection: Connection,
  auth: Keypair
): Promise<PublicKey> {
  const slot = await connection.getSlot(); // Current slot number

  // Instruction to create the address lookup table
  const [lookupTableInstruction, lookupTableAddress] =
    AddressLookupTableProgram.createLookupTable({
      authority: auth.publicKey,
      payer: auth.publicKey,
      recentSlot: slot,
    });

  console.log(`payer: ${auth.publicKey.toBase58()}`);

  // Create a transaction
  const transaction = new Transaction().add(lookupTableInstruction);

  // Send the transaction
  await sendAndConfirmTransaction(connection, transaction, [auth]);

  console.log(
    `Lookup Table Created at Address: ${lookupTableAddress.toBase58()}`
  );
  return lookupTableAddress;
}

async function addAddressesToLookupTable(
  connection: Connection,
  lookupTableAddress: PublicKey,
  auth: Keypair,
  addressesToAdd: PublicKey[]
) {
  const extendInstruction = AddressLookupTableProgram.extendLookupTable({
    payer: auth.publicKey,
    authority: auth.publicKey,
    lookupTable: lookupTableAddress,
    addresses: addressesToAdd,
  });

  const transaction = new Transaction().add(extendInstruction);

  await sendAndConfirmTransaction(connection, transaction, [auth]);

  console.log(
    `Added ${
      addressesToAdd.length
    } addresses to the Lookup Table at ${lookupTableAddress.toBase58()}`
  );
}

const argv = yargs(hideBin(process.argv))
  .option("endpoint-svm", {
    description: "SVM RPC endpoint",
    type: "string",
    demandOption: true,
  })
  .option("sk-auth", {
    description:
      "Secret key of authority for lookup table. In 64-byte base58 format",
    type: "string",
    demandOption: true,
  })
  .option("addresses-to-add", {
    description: "Addresses to add to the lookup table. In base58 format",
    type: "array",
    demandOption: true,
  })
  .option("create-lookup-table", {
    description: "Whether to create a lookup table",
    type: "boolean",
    default: false,
  })
  .option("chain-id", {
    description: "Chain id to create lookup table on.",
    type: "string",
    default: "development-solana",
  })
  .help()
  .alias("help", "h")
  .parseSync();

async function run() {
  const connection = new Connection(argv["endpoint-svm"]);
  const auth = Keypair.fromSecretKey(
    anchor.utils.bytes.bs58.decode(argv["sk-auth"])
  );

  let lookupTableAddr: PublicKey;
  if (argv["create-lookup-table"]) {
    console.log("Creating lookup table...");
    lookupTableAddr = await makeLookupTable(connection, auth);
  } else {
    console.log("Using stored lookup table address...");
    lookupTableAddr = LOOKUP_TABLE_ADDRESS[argv["chain-id"]];
  }

  const addressesToAdd = argv["addresses-to-add"].map(
    (address) => new PublicKey(address)
  );
  if (addressesToAdd.length > 0) {
    await addAddressesToLookupTable(
      connection,
      lookupTableAddr,
      auth,
      addressesToAdd
    );
  }
}

run();
