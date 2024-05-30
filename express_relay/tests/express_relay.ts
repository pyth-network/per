import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { ExpressRelay } from "../target/types/express_relay";
import { EzLend } from "../target/types/ez_lend";
import { OpportunityAdapter } from "../target/types/opportunity_adapter";
import {
  createMint,
  createAccount,
  getAccount,
  getOrCreateAssociatedTokenAccount,
  getAssociatedTokenAddress,
  transfer,
  approve,
  mintTo,
  TOKEN_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
  createWrappedNativeAccount,
  createSyncNativeInstruction,
} from "@solana/spl-token";
import {
  PublicKey,
  AddressLookupTableProgram,
  TransactionMessage,
  VersionedTransaction,
  sendAndConfirmTransaction,
  Ed25519Program,
} from "@solana/web3.js";
import { assert } from "chai";
import { getTxSize } from "./helpers/size_tx";
import { waitForNewBlock } from "./helpers/sleep";
import {
  convertWordArrayToBuffer,
  convertWordArrayToBufferOld,
  wordArrayToByteArray,
  fromWordArray,
} from "./helpers/word_array";
import { sign } from "@noble/ed25519";
import * as crypto from "crypto";

describe("express_relay", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());

  const expressRelay = anchor.workspace.ExpressRelay as Program<ExpressRelay>;
  const ezLend = anchor.workspace.EzLend as Program<EzLend>;
  const opportunityAdapter = anchor.workspace
    .OpportunityAdapter as Program<OpportunityAdapter>;

  const provider = anchor.AnchorProvider.local();
  const LAMPORTS_PER_SOL = 1000000000;
  const payer = anchor.web3.Keypair.generate();
  const mintCollateralAuthority = anchor.web3.Keypair.generate();
  const mintDebtAuthority = anchor.web3.Keypair.generate();

  let mintCollateral;
  let mintDebt;

  let ataCollateralPayer;
  let ataDebtPayer;

  let ataCollateralRelayer;
  let ataDebtRelayer;

  let taCollateralProtocol;
  let taDebtProtocol;

  let expressRelayAuthority;
  let opportunityAdapterAuthority;

  let protocol = ezLend.programId;
  let protocolFeeReceiver;

  const relayerSigner = anchor.web3.Keypair.generate();
  const relayerFeeReceiver = anchor.web3.Keypair.generate();
  const relayerRentReceiver = anchor.web3.Keypair.generate();
  const admin = anchor.web3.Keypair.generate();

  const wsolMint = new PublicKey("So11111111111111111111111111111111111111112");
  let wsolTaUser;
  let wsolTaExpressRelay;

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
      [anchor.utils.bytes.utf8.encode("express_relay_fees")],
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
    ataCollateralRelayer = await getAssociatedTokenAddress(
      mintCollateral,
      relayerSigner.publicKey
    );
    ataDebtRelayer = await getAssociatedTokenAddress(
      mintDebt,
      relayerSigner.publicKey
    );
    taCollateralProtocol = await PublicKey.findProgramAddressSync(
      [anchor.utils.bytes.utf8.encode("ata"), mintCollateral.toBuffer()],
      protocol
    );
    taDebtProtocol = await PublicKey.findProgramAddressSync(
      [anchor.utils.bytes.utf8.encode("ata"), mintDebt.toBuffer()],
      protocol
    );

    expressRelayAuthority = await PublicKey.findProgramAddressSync(
      [anchor.utils.bytes.utf8.encode("authority")],
      expressRelay.programId
    );
    opportunityAdapterAuthority = await PublicKey.findProgramAddressSync(
      [anchor.utils.bytes.utf8.encode("authority")],
      opportunityAdapter.programId
    );

    wsolTaUser = await getOrCreateAssociatedTokenAccount(
      provider.connection,
      payer,
      wsolMint,
      payer.publicKey
    );
    const fundWsolTaUserTx = new anchor.web3.Transaction().add(
      anchor.web3.SystemProgram.transfer({
        fromPubkey: payer.publicKey,
        toPubkey: wsolTaUser.address,
        lamports: 5 * LAMPORTS_PER_SOL,
      }),
      createSyncNativeInstruction(wsolTaUser.address)
    );
    await provider.connection.sendTransaction(fundWsolTaUserTx, [payer]);
    await approve(
      provider.connection,
      payer,
      wsolTaUser.address,
      expressRelayAuthority[0],
      payer.publicKey,
      5 * LAMPORTS_PER_SOL
    );
    wsolTaExpressRelay = await PublicKey.findProgramAddressSync(
      [anchor.utils.bytes.utf8.encode("ata"), wsolMint.toBuffer()],
      expressRelay.programId
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

    // (collateral, protocol)
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
    // (debt, protocol)
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

    // approve user's tokens to opportunity adapter
    await approve(
      provider.connection,
      payer,
      ataCollateralPayer.address,
      opportunityAdapterAuthority[0],
      payer.publicKey,
      1000
    );
    await approve(
      provider.connection,
      payer,
      ataDebtPayer.address,
      opportunityAdapterAuthority[0],
      payer.publicKey,
      10000
    );

    await expressRelay.methods
      .initialize({
        splitProtocolDefault: splitProtocolDefault,
        splitRelayer: splitRelayer,
      })
      .accounts({
        payer: relayerSigner.publicKey,
        expressRelayMetadata: expressRelayMetadata[0],
        admin: admin.publicKey,
        relayerSigner: relayerSigner.publicKey,
        relayerFeeReceiver: relayerFeeReceiver.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([relayerSigner])
      .rpc();
  });

  it("Create and liquidate vault", async () => {
    let vault_id_BN = new anchor.BN(0);
    let collateral_amount = new anchor.BN(100);
    let debt_amount = new anchor.BN(50);

    // get token balances pre
    let balance_collateral_payer_0 = Number(
      (
        await provider.connection.getTokenAccountBalance(
          ataCollateralPayer.address
        )
      ).value.amount
    );
    let balance_debt_payer_0 = Number(
      (await provider.connection.getTokenAccountBalance(ataDebtPayer.address))
        .value.amount
    );
    let balance_collateral_protocol_0 = Number(
      (
        await provider.connection.getTokenAccountBalance(
          taCollateralProtocol[0]
        )
      ).value.amount
    );
    let balance_debt_protocol_0 = Number(
      (await provider.connection.getTokenAccountBalance(taDebtProtocol[0]))
        .value.amount
    );

    // convert the vault id to a bytearray
    let vault_id_bytes = new Uint8Array(32);
    vault_id_bytes.set(vault_id_BN.toArrayLike(Buffer, "le", 32), 0);
    let vault = await PublicKey.findProgramAddressSync(
      [anchor.utils.bytes.utf8.encode("vault"), vault_id_bytes],
      protocol
    );

    const tx_create_vault = await ezLend.methods
      .createVault({
        vaultId: vault_id_bytes,
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
    let balance_collateral_payer_1 = Number(
      (
        await provider.connection.getTokenAccountBalance(
          ataCollateralPayer.address
        )
      ).value.amount
    );
    let balance_debt_payer_1 = Number(
      (await provider.connection.getTokenAccountBalance(ataDebtPayer.address))
        .value.amount
    );
    let balance_collateral_protocol_1 = Number(
      (
        await provider.connection.getTokenAccountBalance(
          taCollateralProtocol[0]
        )
      ).value.amount
    );
    let balance_debt_protocol_1 = Number(
      (await provider.connection.getTokenAccountBalance(taDebtProtocol[0]))
        .value.amount
    );

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
        vaultId: vault_id_bytes,
      })
      .accounts({
        vault: vault[0],
        payer: relayerSigner.publicKey,
        // payer: payer.publicKey,
        collateralMint: mintCollateral,
        debtMint: mintDebt,
        collateralAtaPayer: ataCollateralRelayer,
        // collateralAtaPayer: ataCollateralPayer.address,
        collateralTaProgram: taCollateralProtocol.address,
        debtAtaPayer: ataDebtRelayer,
        // debtAtaPayer: ataDebtPayer.address,
        debtTaProgram: taDebtProtocol.address,
        permission: permission[0],
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([relayerSigner])
      .instruction();

    let bidId: Uint8Array = new Uint8Array(16);
    let bidAmount = new anchor.BN(100_000_000);
    const ixPermission = await expressRelay.methods
      .permission({
        permissionId: vault_id_bytes,
        // bidId: bidId,
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

    const validUntilExpressRelay = new anchor.BN(200_000_000_000_000);
    const msgExpressRelay1 = Uint8Array.from(protocol.toBuffer());
    const msgExpressRelay2 = Uint8Array.from(vault_id_bytes);
    const msgExpressRelay3 = Uint8Array.from(payer.publicKey.toBuffer());
    const msgExpressRelay4 = Uint8Array.from(
      bidAmount.toArrayLike(Buffer, "le", 8)
    );
    const msgExpressRelay5 = Uint8Array.from(
      validUntilExpressRelay.toArrayLike(Buffer, "le", 8)
    );
    console.log(msgExpressRelay1);
    console.log(msgExpressRelay2);
    console.log(msgExpressRelay3);
    console.log(msgExpressRelay4);
    console.log(msgExpressRelay5);
    const msgExpressRelay = Buffer.concat([
      msgExpressRelay1,
      msgExpressRelay2,
      msgExpressRelay3,
      msgExpressRelay4,
      msgExpressRelay5,
    ]);
    const digestExpressRelay = Buffer.from(
      await crypto.subtle.digest("SHA-256", msgExpressRelay)
    );
    const signatureExpressRelay = await sign(
      digestExpressRelay,
      payer.secretKey.slice(0, 32)
    );
    const signatureExpressRelayFirst32 = signatureExpressRelay.slice(0, 32);
    const signatureExpressRelayLast32 = signatureExpressRelay.slice(32, 64);
    let signatureAccountingExpressRelay =
      await PublicKey.findProgramAddressSync(
        [
          anchor.utils.bytes.utf8.encode("signature_accounting"),
          signatureExpressRelayFirst32,
          signatureExpressRelayLast32,
        ],
        expressRelay.programId
      );

    const ixDepermission = await expressRelay.methods
      .depermission({
        permissionId: vault_id_bytes,
        // bidId: bidId,
        signature: signatureExpressRelay,
        validUntil: validUntilExpressRelay,
      })
      .accounts({
        relayerSigner: relayerSigner.publicKey,
        permission: permission[0],
        user: payer.publicKey,
        protocol: protocol,
        protocolFeeReceiver: protocolFeeReceiver[0],
        relayerFeeReceiver: relayerFeeReceiver.publicKey,
        protocolConfig: protocolConfig[0],
        expressRelayMetadata: expressRelayMetadata[0],
        wsolMint: wsolMint,
        wsolTaUser: wsolTaUser.address,
        wsolTaExpressRelay: wsolTaExpressRelay[0],
        expressRelayAuthority: expressRelayAuthority[0],
        signatureAccounting: signatureAccountingExpressRelay[0],
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
        sysvarInstructions: anchor.web3.SYSVAR_INSTRUCTIONS_PUBKEY,
      })
      .signers([relayerSigner])
      .instruction();

    const ixSigVerifyExpressRelay =
      anchor.web3.Ed25519Program.createInstructionWithPublicKey({
        publicKey: payer.publicKey.toBytes(),
        message: digestExpressRelay,
        signature: signatureExpressRelay,
      });

    console.log("DATA FOR EXPRESS RELAY SIG VER");
    console.log(ixSigVerifyExpressRelay.data);
    console.log(ixSigVerifyExpressRelay.data.length);
    console.log(digestExpressRelay);
    console.log(digestExpressRelay.length);
    console.log(signatureExpressRelay);
    console.log(signatureExpressRelay.length);
    console.log(payer.publicKey.toBytes());
    console.log(payer.publicKey.toBytes().length);

    let tokenExpectationCollateral = await PublicKey.findProgramAddressSync(
      [
        anchor.utils.bytes.utf8.encode("token_expectation"),
        payer.publicKey.toBuffer(),
        mintCollateral.toBuffer(),
      ],
      opportunityAdapter.programId
    );

    let tokenExpectationDebt = await PublicKey.findProgramAddressSync(
      [
        anchor.utils.bytes.utf8.encode("token_expectation"),
        payer.publicKey.toBuffer(),
        mintDebt.toBuffer(),
      ],
      opportunityAdapter.programId
    );

    const remainingAccountsOpportunityAdapter = [
      {
        pubkey: mintDebt,
        isWritable: false,
        isSigner: false,
      },
      {
        pubkey: ataDebtPayer.address,
        isWritable: true,
        isSigner: false,
      },
      {
        pubkey: tokenExpectationDebt[0],
        isWritable: true,
        isSigner: false,
      },
      {
        pubkey: ataDebtRelayer,
        isWritable: true,
        isSigner: false,
      },
      {
        pubkey: mintCollateral,
        isWritable: false,
        isSigner: false,
      },
      {
        pubkey: ataCollateralPayer.address,
        isWritable: true,
        isSigner: false,
      },
      {
        pubkey: tokenExpectationCollateral[0],
        isWritable: true,
        isSigner: false,
      },
      {
        pubkey: ataCollateralRelayer,
        isWritable: true,
        isSigner: false,
      },
    ];
    const validUntilOpportunityAdapter = new anchor.BN(100_000_000_000_000);
    const buyTokens = [collateral_amount];
    const buyMints = [mintCollateral];
    const sellTokens = [debt_amount];
    const sellMints = [mintDebt];
    let msgOpportunityAdapter1 = new Uint8Array(2);
    msgOpportunityAdapter1[0] = buyTokens.length;
    msgOpportunityAdapter1[1] = sellTokens.length;
    let msgOpportunityAdapter2 = new Uint8Array(40 * buyTokens.length);
    for (let i = 0; i < buyTokens.length; i++) {
      msgOpportunityAdapter2.set(buyMints[i].toBuffer(), i * 40);
      msgOpportunityAdapter2.set(buyTokens[i].toBuffer(), i * 40 + 32);
    }
    let msgOpportunityAdapter3 = new Uint8Array(40 * sellTokens.length);
    for (let i = 0; i < sellTokens.length; i++) {
      msgOpportunityAdapter3.set(sellMints[i].toBuffer(), i * 40);
      msgOpportunityAdapter3.set(sellTokens[i].toBuffer(), i * 40 + 32);
    }
    const msgOpportunityAdapter4 = Uint8Array.from(payer.publicKey.toBuffer());
    const msgOpportunityAdapter5 = Uint8Array.from(
      validUntilOpportunityAdapter.toBuffer("le", 8)
    );
    const msgOpportunityAdapter = Buffer.concat([
      msgOpportunityAdapter1,
      msgOpportunityAdapter2,
      msgOpportunityAdapter3,
      msgOpportunityAdapter4,
      msgOpportunityAdapter5,
    ]);

    let digestOpportunityAdapter = await crypto.subtle.digest(
      "SHA-256",
      msgOpportunityAdapter
    );
    let digestOpportunityAdapterBuffer = Buffer.from(digestOpportunityAdapter);

    const signatureOpportunityAdapter = await sign(
      digestOpportunityAdapterBuffer,
      payer.secretKey.slice(0, 32)
    );
    const signatureOpportunityAdapterFirst32 =
      signatureOpportunityAdapter.slice(0, 32);
    const signatureOpportnityAdapterLast32 = signatureOpportunityAdapter.slice(
      32,
      64
    );
    let signatureAccountingOpportunityAdapter =
      await PublicKey.findProgramAddressSync(
        [
          anchor.utils.bytes.utf8.encode("signature_accounting"),
          signatureOpportunityAdapterFirst32,
          signatureOpportnityAdapterLast32,
        ],
        opportunityAdapter.programId
      );
    const indexCheckTokenBalances = 4;
    const ixInitializeTokenExpectations = await opportunityAdapter.methods
      .initializeTokenExpectations({
        sellTokens: sellTokens,
        buyTokens: buyTokens,
        indexCheckTokenBalances: indexCheckTokenBalances,
        validUntil: validUntilOpportunityAdapter,
        signature: signatureOpportunityAdapter,
      })
      .accounts({
        relayer: relayerSigner.publicKey,
        user: payer.publicKey,
        opportunityAdapterAuthority: opportunityAdapterAuthority[0],
        signatureAccounting: signatureAccountingOpportunityAdapter[0],
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        sysvarInstructions: anchor.web3.SYSVAR_INSTRUCTIONS_PUBKEY,
      })
      .remainingAccounts(remainingAccountsOpportunityAdapter)
      .signers([relayerSigner])
      .instruction();

    const ixCheckTokenBalances = await opportunityAdapter.methods
      .checkTokenBalances()
      .accounts({
        relayer: relayerSigner.publicKey,
        relayerRentReceiver: relayerRentReceiver.publicKey,
        user: payer.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .remainingAccounts(remainingAccountsOpportunityAdapter)
      .signers([relayerSigner])
      .instruction();

    const ixSigVerifyOpportunityAdapter =
      anchor.web3.Ed25519Program.createInstructionWithPublicKey({
        publicKey: payer.publicKey.toBytes(),
        message: digestOpportunityAdapterBuffer,
        signature: signatureOpportunityAdapter,
      });

    console.log("LENGTH OF IXS' DATA");
    const bytesDataPermission = ixPermission.data.length;
    const bytesDataInitializeTokenExpectations =
      ixInitializeTokenExpectations.data.length;
    const bytesDataLiquidate = ixLiquidate.data.length;
    const bytesDataSigVerifyOpportunityAdapter =
      ixSigVerifyOpportunityAdapter.data.length;
    const bytesDataCheckTokenBalances = ixCheckTokenBalances.data.length;
    const bytesDataSigVerifyExpressRelay = ixSigVerifyExpressRelay.data.length;
    const bytesDataDepermission = ixDepermission.data.length;
    console.log("Permission: ", bytesDataPermission);
    console.log(
      "InitializeTokenExpectations: ",
      bytesDataInitializeTokenExpectations
    );
    console.log("Liquidate: ", bytesDataLiquidate);
    console.log(
      "SigVerifyOpportunityAdapter: ",
      bytesDataSigVerifyOpportunityAdapter
    );
    console.log("CheckTokenBalances: ", bytesDataCheckTokenBalances);
    console.log("SigVerifyExpressRelay: ", bytesDataSigVerifyExpressRelay);
    console.log("Depermission: ", bytesDataDepermission);
    console.log(
      "Total: ",
      bytesDataPermission +
        bytesDataInitializeTokenExpectations +
        bytesDataLiquidate +
        bytesDataSigVerifyOpportunityAdapter +
        bytesDataCheckTokenBalances +
        bytesDataSigVerifyExpressRelay +
        bytesDataDepermission
    );

    // create transaction
    let transaction = new anchor.web3.Transaction();

    transaction.add(ixPermission); // 48, 40 + 8
    transaction.add(ixInitializeTokenExpectations); // 56, variable (98 w 1 buy/1 sell) + 8
    transaction.add(ixLiquidate); // 88, 32 + 8
    transaction.add(ixSigVerifyOpportunityAdapter); // 0, 136 + 8
    if (transaction.instructions.length != indexCheckTokenBalances) {
      throw new Error(
        "Need to match the check token balances ix with the prespecified index"
      );
    }
    transaction.add(ixCheckTokenBalances); // 40, 0 + 8
    transaction.add(ixSigVerifyExpressRelay); // 0, 136 + 8
    transaction.add(ixDepermission); // 120, 104 + 8

    console.log("DATA FOR DEPERMISSION");
    console.log(vault_id_bytes);
    console.log(msgExpressRelay);
    console.log("DIGEST EXPRESS RELAY", digestExpressRelay);
    console.log(signatureExpressRelay);
    console.log(validUntilExpressRelay.toNumber());
    console.log(ixDepermission.data);

    let solProtocolPre = await provider.connection.getBalance(
      protocolFeeReceiver[0]
    );
    let solRelayerRentReceiverPre = await provider.connection.getBalance(
      relayerRentReceiver.publicKey
    );
    let solRelayerFeeReceiverPre = await provider.connection.getBalance(
      relayerFeeReceiver.publicKey
    );
    let solExpressRelayPre = await provider.connection.getBalance(
      expressRelayMetadata[0]
    );

    console.log(
      "SIZE of transaction (no lookup tables): ",
      getTxSize(transaction, relayerSigner.publicKey)
    );

    // get lookup table accounts
    const accounts = new Set<PublicKey>();
    const accounts2 = new Set<PublicKey>();
    transaction.instructions.reduce((acc, ix) => {
      ix.keys.forEach(({ pubkey, isSigner }) => {
        if (accounts.size < 30) {
          accounts.add(pubkey);
        } else if (!accounts.has(pubkey)) {
          accounts2.add(pubkey);
        }
      });

      accounts2.add(ix.programId);

      return accounts.size;
    }, 0);
    console.log("LENGTH OF ACCOUNTS: ", accounts.size);
    console.log("LENGTH OF ACCOUNTS2: ", accounts2.size);

    // create Lookup table
    let transactionLookupTable = new anchor.web3.Transaction();
    let slot = (await provider.connection.getSlot()) - 1;
    // const slots = await provider.connection.getBlocks(slot - 20);
    // console.log(slots);
    const [lookupTableInst, lookupTableAddress] =
      AddressLookupTableProgram.createLookupTable({
        authority: relayerSigner.publicKey,
        payer: relayerSigner.publicKey,
        recentSlot: slot,
      });
    transactionLookupTable.add(lookupTableInst);
    const extendInstruction = AddressLookupTableProgram.extendLookupTable({
      payer: relayerSigner.publicKey,
      authority: relayerSigner.publicKey,
      lookupTable: lookupTableAddress,
      addresses: Array.from(accounts),
    });
    transactionLookupTable.add(extendInstruction);
    console.log(
      "SIZE of transaction (create lookup tables): ",
      getTxSize(transactionLookupTable, relayerSigner.publicKey)
    );
    let signatureLookupTable = await provider.connection
      .sendTransaction(transactionLookupTable, [relayerSigner], {})
      .catch((err) => {
        console.log(err);
      });
    const latestBlockHashLookupTable =
      await provider.connection.getLatestBlockhash();
    let txResponseLookupTable = await provider.connection.confirmTransaction({
      blockhash: latestBlockHashLookupTable.blockhash,
      lastValidBlockHeight: latestBlockHashLookupTable.lastValidBlockHeight,
      signature: signatureLookupTable,
    });
    console.log("Lookup table created");

    // add more to Lookup table
    let transactionLookupTable2 = new anchor.web3.Transaction();
    let slot2 = (await provider.connection.getSlot()) - 1;
    const extendInstruction2 = AddressLookupTableProgram.extendLookupTable({
      payer: relayerSigner.publicKey,
      authority: relayerSigner.publicKey,
      lookupTable: lookupTableAddress,
      addresses: Array.from(accounts2),
    });
    transactionLookupTable2.add(extendInstruction2);
    console.log(
      "SIZE of transaction (add to lookup tables): ",
      getTxSize(transactionLookupTable2, relayerSigner.publicKey)
    );
    let signatureLookupTable2 = await provider.connection
      .sendTransaction(transactionLookupTable2, [relayerSigner], {})
      .catch((err) => {
        console.log(err);
      });
    const latestBlockHashLookupTable2 =
      await provider.connection.getLatestBlockhash();
    let txResponseLookupTable2 = await provider.connection.confirmTransaction({
      blockhash: latestBlockHashLookupTable2.blockhash,
      lastValidBlockHeight: latestBlockHashLookupTable2.lastValidBlockHeight,
      signature: signatureLookupTable2,
    });
    console.log("Lookup table added to");

    // sleep to allow the lookup table to activate
    await waitForNewBlock(provider.connection, 2);

    // construct original tx with lookup table
    const lookupTableAccount = (
      await provider.connection.getAddressLookupTable(lookupTableAddress)
    ).value;
    const latestBlockHash = await provider.connection.getLatestBlockhash();
    const messageV0 = new TransactionMessage({
      payerKey: relayerSigner.publicKey,
      recentBlockhash: latestBlockHash.blockhash,
      instructions: transaction.instructions, // note this is an array of instructions
    }).compileToV0Message([lookupTableAccount]);

    // create a v0 transaction from the v0 message
    const transactionV0 = new VersionedTransaction(messageV0);
    console.log(
      "JSON Stringified tx (legacy) object: ",
      JSON.stringify(transaction)
    );
    console.log(
      "JSON Stringified tx (V0) object: ",
      JSON.stringify(transactionV0)
    );
    console.log("LENGTH OF versioned tx: ", messageV0.serialize().length);

    // sign the v0 transaction
    transactionV0.sign([relayerSigner]);

    // send and confirm the transaction
    // const txResponse = await provider.connection.sendTransaction(transactionV0); // {skipPreflight: true}
    const txResponse = await sendAndConfirmTransaction(
      provider.connection,
      transactionV0
    ).catch((err) => {
      console.log(err);
    }); // {skipPreflight: true}

    let solProtocolPost = await provider.connection.getBalance(
      protocolFeeReceiver[0]
    );
    let solRelayerRentReceiverPost = await provider.connection.getBalance(
      relayerRentReceiver.publicKey
    );
    let solRelayerFeeReceiverPost = await provider.connection.getBalance(
      relayerFeeReceiver.publicKey
    );
    let solExpressRelayPost = await provider.connection.getBalance(
      expressRelayMetadata[0]
    );

    // get token balances post liquidation
    let balance_collateral_payer_2 = Number(
      (
        await provider.connection.getTokenAccountBalance(
          ataCollateralPayer.address
        )
      ).value.amount
    );
    let balance_debt_payer_2 = Number(
      (await provider.connection.getTokenAccountBalance(ataDebtPayer.address))
        .value.amount
    );
    let balance_collateral_protocol_2 = Number(
      (
        await provider.connection.getTokenAccountBalance(
          taCollateralProtocol[0]
        )
      ).value.amount
    );
    let balance_debt_protocol_2 = Number(
      (await provider.connection.getTokenAccountBalance(taDebtProtocol[0]))
        .value.amount
    );

    console.log("TX RESPONSE", txResponse);

    assert(
      balance_collateral_payer_1 ==
        balance_collateral_payer_0 - collateral_amount.toNumber()
    );
    assert(
      balance_debt_payer_1 == balance_debt_payer_0 + debt_amount.toNumber()
    );
    assert(
      balance_collateral_protocol_1 ==
        balance_collateral_protocol_0 + collateral_amount.toNumber()
    );
    assert(
      balance_debt_protocol_1 ==
        balance_debt_protocol_0 - debt_amount.toNumber()
    );

    console.log(
      "BALANCES, COLLATERAL, USER",
      balance_collateral_payer_2,
      balance_collateral_payer_1,
      collateral_amount
    );
    console.log(
      "BALANCES, DEBT, USER",
      balance_debt_payer_2,
      balance_debt_payer_1,
      debt_amount
    );
    console.log(
      "BALANCES, COLLATERAL, PROTOCOL",
      balance_collateral_protocol_2,
      balance_collateral_protocol_1,
      collateral_amount
    );
    console.log(
      "BALANCES, DEBT, PROTOCOL",
      balance_debt_protocol_2,
      balance_debt_protocol_1,
      debt_amount
    );
    assert(
      balance_collateral_payer_2 ==
        balance_collateral_payer_1 + collateral_amount.toNumber()
    );
    assert(
      balance_debt_payer_2 == balance_debt_payer_1 - debt_amount.toNumber()
    );
    assert(
      balance_collateral_protocol_2 ==
        balance_collateral_protocol_1 - collateral_amount.toNumber()
    );
    assert(
      balance_debt_protocol_2 ==
        balance_debt_protocol_1 + debt_amount.toNumber()
    );
  });
});
