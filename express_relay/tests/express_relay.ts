import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { ExpressRelay } from "../target/types/express_relay";
import { EzLend } from "../target/types/ez_lend";
import {
  createMint,
  createAccount,
  getAccount,
  getOrCreateAssociatedTokenAccount,
  transfer,
  mintTo,
  TOKEN_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import { PublicKey } from "@solana/web3.js";
// import { BN } from "bn.js";

describe("express_relay", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());

  const expressRelay = anchor.workspace.ExpressRelay as Program<ExpressRelay>;
  const ezLend = anchor.workspace.EzLend as Program<EzLend>;

  const provider = anchor.AnchorProvider.local();
  const LAMPORTS_PER_SOL = 1000000000;
  const payer = anchor.web3.Keypair.generate();
  const mintCollateralAuthority = anchor.web3.Keypair.generate();
  const mintDebtAuthority = anchor.web3.Keypair.generate();

  let mintCollateral;
  let mintDebt;

  let ataCollateralPayer;
  let ataDebtPayer;

  let taCollateralProtocol;
  let taDebtProtocol;

  let protocol = ezLend.programId;
  let protocolFeeReceiver;

  const relayerSigner = anchor.web3.Keypair.generate();
  const relayerFeeReceiver = anchor.web3.Keypair.generate();
  const admin = anchor.web3.Keypair.generate();
  let expressRelayMetadata;
  let splitProtocolDefault = new anchor.BN(5000);
  let splitRelayer = new anchor.BN(2000);

  console.log("payer: ", payer.publicKey.toBase58());
  console.log("relayerSigner: ", relayerSigner.publicKey.toBase58());
  console.log("relayerFeeReceiver: ", relayerFeeReceiver.publicKey.toBase58());
  console.log("admin: ", admin.publicKey.toBase58());

  before(async () => {
    let airdrop_signature_payer = await provider.connection.requestAirdrop(
      payer.publicKey,
      20 * LAMPORTS_PER_SOL
    );
    await provider.connection.confirmTransaction(airdrop_signature_payer);

    let airdrop_signature_relayer_signer =
      await provider.connection.requestAirdrop(
        relayerSigner.publicKey,
        30 * LAMPORTS_PER_SOL
      );
    await provider.connection.confirmTransaction(
      airdrop_signature_relayer_signer
    );

    // create mints
    mintCollateral = await createMint(
      provider.connection,
      payer,
      mintCollateralAuthority.publicKey,
      mintCollateralAuthority.publicKey,
      9
    );
    mintDebt = await createMint(
      provider.connection,
      payer,
      mintDebtAuthority.publicKey,
      mintDebtAuthority.publicKey,
      9
    );

    protocolFeeReceiver = await PublicKey.findProgramAddressSync(
      [anchor.utils.bytes.utf8.encode("per_fees")],
      protocol
    );

    // Initialize TAs
    ataCollateralPayer = await getOrCreateAssociatedTokenAccount(
      provider.connection,
      payer,
      mintCollateral,
      payer.publicKey
    );
    ataDebtPayer = await getOrCreateAssociatedTokenAccount(
      provider.connection,
      payer,
      mintDebt,
      payer.publicKey
    );
    taCollateralProtocol = await PublicKey.findProgramAddressSync(
      [anchor.utils.bytes.utf8.encode("ata"), mintCollateral.toBuffer()],
      protocol
    );
    taDebtProtocol = await PublicKey.findProgramAddressSync(
      [anchor.utils.bytes.utf8.encode("ata"), mintDebt.toBuffer()],
      protocol
    );

    expressRelayMetadata = await PublicKey.findProgramAddressSync(
      [anchor.utils.bytes.utf8.encode("metadata")],
      expressRelay.programId
    );

    const tx_collateral_ta = await ezLend.methods
      .createTokenAcc({})
      .accounts({
        payer: payer.publicKey,
        mint: mintCollateral,
        tokenAccount: taCollateralProtocol[0],
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([payer])
      .rpc();

    const tx_debt_ta = await ezLend.methods
      .createTokenAcc({})
      .accounts({
        payer: payer.publicKey,
        mint: mintDebt,
        tokenAccount: taDebtProtocol[0],
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([payer])
      .rpc();

    // (collateral, payer)
    await mintTo(
      provider.connection,
      payer,
      mintCollateral,
      ataCollateralPayer.address,
      mintCollateralAuthority,
      1000,
      [],
      undefined,
      TOKEN_PROGRAM_ID
    );
    // (debt, payer)
    await mintTo(
      provider.connection,
      payer,
      mintDebt,
      ataDebtPayer.address,
      mintDebtAuthority,
      1000,
      [],
      undefined,
      TOKEN_PROGRAM_ID
    );

    console.log(ataDebtPayer.address);
    console.log(taCollateralProtocol[0]);

    // (collateral, payer)
    await mintTo(
      provider.connection,
      payer,
      mintCollateral,
      taCollateralProtocol[0],
      mintCollateralAuthority,
      10000,
      [],
      undefined,
      TOKEN_PROGRAM_ID
    );
    // (debt, payer)
    await mintTo(
      provider.connection,
      payer,
      mintDebt,
      taDebtProtocol[0],
      mintDebtAuthority,
      10000,
      [],
      undefined,
      TOKEN_PROGRAM_ID
    );

    await expressRelay.methods
      .initialize({
        splitProtocolDefault: splitProtocolDefault,
        splitRelayer: splitRelayer,
      })
      .accounts({
        payer: relayerSigner.publicKey,
        express_relay_metadata: expressRelayMetadata[0],
        admin: admin.publicKey,
        relayerSigner: relayerSigner.publicKey,
        relayerFeeReceiver: relayerFeeReceiver.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([relayerSigner])
      .rpc();
  });

  it("Create vault", async () => {
    let vault_id = 0;
    let vault_id_BN = new anchor.BN(vault_id);
    let collateral_amount = new anchor.BN(100);
    let debt_amount = new anchor.BN(50);

    // get token balances pre
    let balance_collateral_payer_0 =
      await provider.connection.getTokenAccountBalance(
        ataCollateralPayer.address
      );
    let balance_debt_payer_0 = await provider.connection.getTokenAccountBalance(
      ataDebtPayer.address
    );
    let balance_collateral_protocol_0 =
      await provider.connection.getTokenAccountBalance(taCollateralProtocol[0]);
    let balance_debt_protocol_0 =
      await provider.connection.getTokenAccountBalance(taDebtProtocol[0]);

    // convert the vault id to a bytearray
    let vault_id_bytes = new Uint8Array(8);
    vault_id_bytes.set(
      new Uint8Array(new BigUint64Array([BigInt(vault_id)]).buffer)
    );
    let vault = await PublicKey.findProgramAddressSync(
      [anchor.utils.bytes.utf8.encode("vault"), vault_id_bytes],
      protocol
    );

    const tx_create_vault = await ezLend.methods
      .createVault({
        vaultId: vault_id_BN,
        collateralAmount: collateral_amount,
        debtAmount: debt_amount,
      })
      .accounts({
        vault: vault[0],
        payer: payer.publicKey,
        collateralMint: mintCollateral,
        debtMint: mintDebt,
        collateralAtaPayer: ataCollateralPayer.address,
        collateralTaProgram: taCollateralProtocol.address,
        debtAtaPayer: ataDebtPayer.address,
        debtTaProgram: taDebtProtocol.address,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([payer])
      .rpc();

    // get token balances post creation
    let balance_collateral_payer_1 =
      await provider.connection.getTokenAccountBalance(
        ataCollateralPayer.address
      );
    let balance_debt_payer_1 = await provider.connection.getTokenAccountBalance(
      ataDebtPayer.address
    );
    let balance_collateral_protocol_1 =
      await provider.connection.getTokenAccountBalance(taCollateralProtocol[0]);
    let balance_debt_protocol_1 =
      await provider.connection.getTokenAccountBalance(taDebtProtocol[0]);

    let permission = await PublicKey.findProgramAddressSync(
      [
        anchor.utils.bytes.utf8.encode("permission"),
        protocol.toBuffer(),
        vault_id_bytes,
      ],
      expressRelay.programId
    );

    const ixLiquidate = await ezLend.methods
      .liquidate({
        vaultId: vault_id_BN,
      })
      .accounts({
        vault: vault[0],
        payer: payer.publicKey,
        collateralMint: mintCollateral,
        debtMint: mintDebt,
        collateralAtaPayer: ataCollateralPayer.address,
        collateralTaProgram: taCollateralProtocol.address,
        debtAtaPayer: ataDebtPayer.address,
        debtTaProgram: taDebtProtocol.address,
        expressRelay: expressRelay.programId,
        permission: permission[0],
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([payer])
      .instruction();

    let bidId: Uint8Array = new Uint8Array(16);
    let bidAmount = new anchor.BN(100_000_000);
    console.log("permission", permission[0]);
    console.log("vault ID", vault_id_BN);
    console.log("vault ID buffer", vault_id_bytes);
    const ixPermission = await expressRelay.methods
      .permission({
        permissionId: vault_id_bytes,
        bidId: bidId,
        bidAmount: bidAmount,
      })
      .accounts({
        relayerSigner: relayerSigner.publicKey,
        permission: permission[0],
        protocol: protocol,
        expressRelayMetadata: expressRelayMetadata[0],
        systemProgram: anchor.web3.SystemProgram.programId,
        sysvarInstructions: anchor.web3.SYSVAR_INSTRUCTIONS_PUBKEY,
      })
      .signers([relayerSigner])
      .instruction();

    let protocolConfig = await PublicKey.findProgramAddressSync(
      [anchor.utils.bytes.utf8.encode("config_protocol"), protocol.toBuffer()],
      expressRelay.programId
    );

    const ixDepermission = await expressRelay.methods
      .depermission({
        permissionId: vault_id_bytes,
        bidId: bidId,
      })
      .accounts({
        relayerSigner: relayerSigner.publicKey,
        permission: permission[0],
        protocol: ezLend.programId,
        protocolFeeReceiver: protocolFeeReceiver[0],
        relayerFeeReceiver: relayerFeeReceiver.publicKey,
        protocolConfig: protocolConfig[0],
        expressRelayMetadata: expressRelayMetadata[0],
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([relayerSigner])
      .instruction();

    const ixSendSol = anchor.web3.SystemProgram.transfer({
      fromPubkey: payer.publicKey,
      toPubkey: permission[0],
      lamports: bidAmount.toNumber(),
    });

    // create transaction
    let transaction = new anchor.web3.Transaction();

    transaction.add(ixPermission);
    transaction.add(ixLiquidate);
    transaction.add(ixSendSol);
    transaction.add(ixDepermission);

    let solProtocolPre = await provider.connection.getBalance(
      protocolFeeReceiver[0]
    );
    let solRelayerPre = await provider.connection.getBalance(
      relayerFeeReceiver.publicKey
    );
    let solExpressRelayPre = await provider.connection.getBalance(
      expressRelayMetadata[0]
    );

    // send transaction
    let signature = await provider.connection.sendTransaction(
      transaction,
      [payer, relayerSigner],
      {}
    );

    const latestBlockHash = await provider.connection.getLatestBlockhash();
    let txResponse = await provider.connection.confirmTransaction({
      blockhash: latestBlockHash.blockhash,
      lastValidBlockHeight: latestBlockHash.lastValidBlockHeight,
      signature: signature,
    });
    console.log("Transaction response", txResponse.value["err"]);

    let solProtocolPost = await provider.connection.getBalance(
      protocolFeeReceiver[0]
    );
    let solRelayerPost = await provider.connection.getBalance(
      relayerFeeReceiver.publicKey
    );
    let solExpressRelayPost = await provider.connection.getBalance(
      expressRelayMetadata[0]
    );

    // get token balances post creation
    let balance_collateral_payer_2 =
      await provider.connection.getTokenAccountBalance(
        ataCollateralPayer.address
      );
    let balance_debt_payer_2 = await provider.connection.getTokenAccountBalance(
      ataDebtPayer.address
    );
    let balance_collateral_protocol_2 =
      await provider.connection.getTokenAccountBalance(taCollateralProtocol[0]);
    let balance_debt_protocol_2 =
      await provider.connection.getTokenAccountBalance(taDebtProtocol[0]);

    console.log("SOL balance change (protocol)");
    console.log(solProtocolPre);
    console.log(solProtocolPost);

    console.log("SOL balance change (relayer)");
    console.log(solRelayerPre);
    console.log(solRelayerPost);

    console.log("SOL balance change (express relay)");
    console.log(solExpressRelayPre);
    console.log(solExpressRelayPost);

    console.log("BEFORE CREATION");
    console.log(balance_collateral_payer_0.value.amount);
    console.log(balance_debt_payer_0.value.amount);
    console.log(balance_collateral_protocol_0.value.amount);
    console.log(balance_debt_protocol_0.value.amount);

    console.log("BEFORE LIQ");
    console.log(balance_collateral_payer_1.value.amount);
    console.log(balance_debt_payer_1.value.amount);
    console.log(balance_collateral_protocol_1.value.amount);
    console.log(balance_debt_protocol_1.value.amount);

    console.log("AFTER LIQ");
    console.log(balance_collateral_payer_2.value.amount);
    console.log(balance_debt_payer_2.value.amount);
    console.log(balance_collateral_protocol_2.value.amount);
    console.log(balance_debt_protocol_2.value.amount);
  });
});
