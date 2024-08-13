import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { ExpressRelay } from "../target/types/express_relay";
import { Dummy } from "../target/types/dummy";

import * as fs from "fs";
import { assert } from "chai";
import { PublicKey } from "@solana/web3.js";
import { TOKEN_PROGRAM_ID } from "@coral-xyz/anchor/dist/cjs/utils/token";

import {
  writeKeypairToFile,
  readKeypairFromFile,
} from "./helpers/keypairUtils";
import { createAndSubmitTransaction } from "./helpers/utils";

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

  const ownerDummy = anchor.web3.Keypair.generate();
  const feeReceiverDummy = PublicKey.findProgramAddressSync(
    [anchor.utils.bytes.utf8.encode("express_relay_fees")],
    dummy.programId
  )[0];

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

    let airdropSignatureSearcher = await provider.connection.requestAirdrop(
      searcher.publicKey,
      20 * LAMPORTS_PER_SOL
    );
    await provider.connection.confirmTransaction(airdropSignatureSearcher);

    let airdropSignatureRelayerSigner =
      await provider.connection.requestAirdrop(
        relayerSigner.publicKey,
        30 * LAMPORTS_PER_SOL
      );
    await provider.connection.confirmTransaction(airdropSignatureRelayerSigner);

    let airdropSignatureFeeReceiverRelayer =
      await provider.connection.requestAirdrop(
        feeReceiverRelayer.publicKey,
        10 * LAMPORTS_PER_SOL
      );
    await provider.connection.confirmTransaction(
      airdropSignatureFeeReceiverRelayer
    );

    let airdropOwnerDummy = await provider.connection.requestAirdrop(
      ownerDummy.publicKey,
      1 * LAMPORTS_PER_SOL
    );
    await provider.connection.confirmTransaction(airdropOwnerDummy);

    let airdropSignatureAdmin = await provider.connection.requestAirdrop(
      admin.publicKey,
      20 * LAMPORTS_PER_SOL
    );
    await provider.connection.confirmTransaction(airdropSignatureAdmin);
  });

  before(async () => {
    console.log("INITIALIZING EXPRESS RELAY");

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

  before(async () => {
    console.log("INITIALIZING DUMMY");

    const balanceFeeReceiverDummy = await provider.connection.getBalance(
      feeReceiverDummy
    );
    if (balanceFeeReceiverDummy === 0) {
      await dummy.methods
        .initialize()
        .accountsPartial({
          payer: ownerDummy.publicKey,
          feeReceiverExpressRelay: feeReceiverDummy,
          systemProgram: anchor.web3.SystemProgram.programId,
        })
        .signers([ownerDummy])
        .rpc();
    } else {
      console.debug("Dummy already initialized");
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

    const lookupAccounts = [
      relayerSigner.publicKey,
      dummy.programId,
      protocolConfigDummy,
      feeReceiverRelayer.publicKey,
      feeReceiverDummy,
      expressRelayMetadata[0],
      anchor.web3.SystemProgram.programId,
      TOKEN_PROGRAM_ID,
      anchor.web3.SYSVAR_INSTRUCTIONS_PUBKEY,
      expressRelay.programId,
      dummy.programId,
    ];
    const lookupPayer = relayerSigner;
    const payer = searcher.publicKey;
    const signers = [searcher, relayerSigner];
    const [txHash, txFee] = await createAndSubmitTransaction(
      provider.connection,
      [ixPermission, ixDoNothing],
      lookupAccounts,
      lookupPayer,
      payer,
      signers,
      false
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

    let feeDummy =
      (bidAmount.toNumber() * splitProtocolDefault.toNumber()) / 10000;
    let feeRelayer =
      ((bidAmount.toNumber() - feeDummy) * splitRelayer.toNumber()) / 10000;
    let feeExpressRelay = bidAmount.toNumber() - feeDummy - feeRelayer;

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
      feeDummy,
      "Protocol fee receiver balance should be increased by its share of the bid"
    );

    assert.equal(
      balanceRelayerPost - balanceRelayerPre,
      feeRelayer,
      "Relayer fee receiver balance should be increased by its share of the bid"
    );

    assert.equal(
      balanceExpressRelayPost - balanceExpressRelayPre,
      feeExpressRelay,
      "Express Relay balance should be increased by its share of the bid"
    );
  });
});
