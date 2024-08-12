import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { ExpressRelay } from "../target/types/express_relay";
import { Dummy } from "../target/types/dummy";

import * as fs from "fs";
import { assert } from "chai";
import {
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import { TOKEN_PROGRAM_ID } from "@coral-xyz/anchor/dist/cjs/utils/token";

import {
  writeKeypairToFile,
  readKeypairFromFile,
} from "./helpers/keypairUtils";
import { sendAndConfirmVersionedTransaction } from "./helpers/utils";

describe("express_relay", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());

  const expressRelay = anchor.workspace.ExpressRelay as Program<ExpressRelay>;
  const dummy = anchor.workspace.Dummy as Program<Dummy>;

  const provider = anchor.AnchorProvider.local();
  const LAMPORTS_PER_SOL = 1000000000;

  const searcher = anchor.web3.Keypair.generate();
  let relayerSigner: anchor.web3.Keypair;
  let feeReceiverRelayer: anchor.web3.Keypair;
  let admin: anchor.web3.Keypair;

  const expressRelayMetadata = PublicKey.findProgramAddressSync(
    [anchor.utils.bytes.utf8.encode("metadata")],
    expressRelay.programId
  );

  const splitProtocolDefault = new anchor.BN(5000);
  const splitRelayer = new anchor.BN(2000);

  before(async () => {
    console.log("GETTING/GENERATING KEYS");

    if (!fs.existsSync("tests/keys/relayerSigner.json")) {
      relayerSigner = anchor.web3.Keypair.generate();
      await writeKeypairToFile(
        relayerSigner.secretKey,
        "tests/keys/relayerSigner.json"
      );
    } else {
      relayerSigner = await readKeypairFromFile(
        "tests/keys/relayerSigner.json"
      );
    }

    if (!fs.existsSync("tests/keys/feeReceiverRelayer.json")) {
      feeReceiverRelayer = anchor.web3.Keypair.generate();
      await writeKeypairToFile(
        feeReceiverRelayer.secretKey,
        "tests/keys/feeReceiverRelayer.json"
      );
    } else {
      feeReceiverRelayer = await readKeypairFromFile(
        "tests/keys/feeReceiverRelayer.json"
      );
    }

    if (!fs.existsSync("tests/keys/admin.json")) {
      admin = anchor.web3.Keypair.generate();
      await writeKeypairToFile(admin.secretKey, "tests/keys/admin.json");
    } else {
      admin = await readKeypairFromFile("tests/keys/admin.json");
    }
  });

  before(async () => {
    console.log("FUNDING");

    let airdrop_signature_searcher = await provider.connection.requestAirdrop(
      searcher.publicKey,
      20 * LAMPORTS_PER_SOL
    );
    await provider.connection.confirmTransaction(airdrop_signature_searcher);

    let airdrop_signature_relayer_signer =
      await provider.connection.requestAirdrop(
        relayerSigner.publicKey,
        30 * LAMPORTS_PER_SOL
      );
    await provider.connection.confirmTransaction(
      airdrop_signature_relayer_signer
    );

    let airdrop_signature_admin = await provider.connection.requestAirdrop(
      admin.publicKey,
      20 * LAMPORTS_PER_SOL
    );
    await provider.connection.confirmTransaction(airdrop_signature_admin);
  });

  before(async () => {
    console.log("INITIALIZING");

    const balanceExpressRelayMetadata = await provider.connection.getBalance(
      expressRelayMetadata[0]
    );
    if (balanceExpressRelayMetadata === 0) {
      await expressRelay.methods
        .initialize({
          splitProtocolDefault: splitProtocolDefault,
          splitRelayer: splitRelayer,
        })
        .accountsPartial({
          payer: relayerSigner.publicKey,
          expressRelayMetadata: expressRelayMetadata[0],
          admin: admin.publicKey,
          relayerSigner: relayerSigner.publicKey,
          feeReceiverRelayer: feeReceiverRelayer.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
        })
        .signers([relayerSigner])
        .rpc();
    } else {
      console.debug("Express Relay already initialized");
    }
  });

  it("Dummy:DoNothing via ExpressRelay", async () => {
    const permission = anchor.web3.Keypair.generate().publicKey;
    const deadline = new anchor.BN(1_000_000_000_000_000);
    const bidAmount = new anchor.BN(1_000_000);

    const protocolConfigDummy = PublicKey.findProgramAddressSync(
      [
        anchor.utils.bytes.utf8.encode("config_protocol"),
        dummy.programId.toBuffer(),
      ],
      expressRelay.programId
    )[0];
    const feeReceiverDummy = PublicKey.findProgramAddressSync(
      [anchor.utils.bytes.utf8.encode("express_relay_fees")],
      dummy.programId
    )[0];

    let balanceSearcherPre = await provider.connection.getBalance(
      searcher.publicKey
    );
    let balanceDummyPre = await provider.connection.getBalance(
      feeReceiverDummy
    );
    let balanceRelayerPre = await provider.connection.getBalance(
      feeReceiverRelayer.publicKey
    );
    let balanceExpressRelayPre = await provider.connection.getBalance(
      expressRelayMetadata[0]
    );

    const ixPermission = await expressRelay.methods
      .permission({
        deadline: deadline,
        bidAmount: bidAmount,
      })
      .accountsPartial({
        relayerSigner: relayerSigner.publicKey,
        searcher: searcher.publicKey,
        permission: permission,
        protocol: dummy.programId,
        protocolConfig: protocolConfigDummy,
        feeReceiverRelayer: feeReceiverRelayer.publicKey,
        feeReceiverProtocol: feeReceiverDummy,
        expressRelayMetadata: expressRelayMetadata[0],
        systemProgram: anchor.web3.SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
        sysvarInstructions: anchor.web3.SYSVAR_INSTRUCTIONS_PUBKEY,
      })
      .signers([relayerSigner, searcher])
      .instruction();

    const ixDoNothing = await dummy.methods
      .doNothing()
      .accountsPartial({
        payer: searcher.publicKey,
        expressRelay: expressRelay.programId,
        sysvarInstructions: anchor.web3.SYSVAR_INSTRUCTIONS_PUBKEY,
        permission: permission,
        protocol: dummy.programId,
      })
      .signers([searcher])
      .instruction();

    let transaction = new anchor.web3.Transaction();

    transaction.add(ixPermission);
    transaction.add(ixDoNothing);

    const lookupTables = [];
    const latestBlockHash = await provider.connection.getLatestBlockhash();
    const messageV0 = new TransactionMessage({
      payerKey: searcher.publicKey,
      recentBlockhash: latestBlockHash.blockhash,
      instructions: transaction.instructions,
    }).compileToV0Message(lookupTables);

    let txFee = (await provider.connection.getFeeForMessage(messageV0)).value;

    const transactionV0 = new VersionedTransaction(messageV0);
    transactionV0.sign([relayerSigner, searcher]);
    await sendAndConfirmVersionedTransaction(
      provider.connection,
      transactionV0
    );

    let balanceSearcherPost = await provider.connection.getBalance(
      searcher.publicKey
    );
    let balanceDummyPost = await provider.connection.getBalance(
      feeReceiverDummy
    );
    let balanceRelayerPost = await provider.connection.getBalance(
      feeReceiverRelayer.publicKey
    );
    let balanceExpressRelayPost = await provider.connection.getBalance(
      expressRelayMetadata[0]
    );

    let rentFeeReceiverDummy =
      await provider.connection.getMinimumBalanceForRentExemption(
        0 // feeReceiverDummy should be of length 0
      );
    let feeDummy =
      (bidAmount.toNumber() * splitProtocolDefault.toNumber()) / 10000;

    let rentFeeReceiverRelayer =
      await provider.connection.getMinimumBalanceForRentExemption(
        0 // feeReceiverRelayer should be of length 0
      );
    let feeRelayer =
      ((bidAmount.toNumber() - feeDummy) * splitRelayer.toNumber()) / 10000;

    let feeExpressRelay = bidAmount.toNumber() - feeDummy - feeRelayer;

    (1 * LAMPORTS_PER_SOL) / 1e5;

    assert.equal(
      balanceSearcherPre - balanceSearcherPost,
      balanceDummyPost -
        balanceDummyPre +
        (balanceRelayerPost - balanceRelayerPre) +
        (balanceExpressRelayPost - balanceExpressRelayPre) +
        txFee,
      "Searcher balance should be deducted by the increases in fee recipients' accounts + the tx fee"
    );

    assert.equal(
      balanceDummyPost - balanceDummyPre,
      rentFeeReceiverDummy + feeDummy,
      "Protocol fee receiver balance should be increased by its share of the bid plus rent"
    );

    assert.equal(
      balanceRelayerPost - balanceRelayerPre,
      rentFeeReceiverRelayer + feeRelayer,
      "Relayer fee receiver balance should be increased by its share of the bid plus rent"
    );

    assert.equal(
      balanceExpressRelayPost - balanceExpressRelayPre,
      feeExpressRelay,
      "Express Relay balance should be increased by its share of the bid"
    );
  });
});
