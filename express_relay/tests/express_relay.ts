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
import { BN } from "bn.js";

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

  before(async () => {
    let airdrop_signature = await provider.connection.requestAirdrop(
      payer.publicKey,
      2 * LAMPORTS_PER_SOL
    );
    await provider.connection.confirmTransaction(airdrop_signature);

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
      ezLend.programId
    );
    taDebtProtocol = await PublicKey.findProgramAddressSync(
      [anchor.utils.bytes.utf8.encode("ata"), mintDebt.toBuffer()],
      ezLend.programId
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

    // const tx = await ezLend.methods.
    //   initialize({}).
    //   accounts({
    //     payer: payer.publicKey,
    //     ataAuthorityProgram: authorityProtocol[0],
    //     systemProgram: anchor.web3.SystemProgram.programId
    //   }).
    //   signers([payer]).
    //   rpc();
  });

  it("Create vault", async () => {
    let vault_id = 0;
    let vault_id_BN = new BN(vault_id);
    let collateral_amount = new BN(100);
    let debt_amount = new BN(50);

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
      ezLend.programId
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
        anchor.utils.bytes.utf8.encode("ata"),
        ezLend.programId.toBuffer(),
        vault[0].toBuffer(),
      ],
      expressRelay.programId
    );

    const tx_liquidate = await ezLend.methods
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
      .rpc();

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

    console.log(balance_collateral_payer_0.value.amount);
    console.log(balance_debt_payer_0.value.amount);
    console.log(balance_collateral_protocol_0.value.amount);
    console.log(balance_debt_protocol_0.value.amount);

    console.log(balance_collateral_payer_1.value.amount);
    console.log(balance_debt_payer_1.value.amount);
    console.log(balance_collateral_protocol_1.value.amount);
    console.log(balance_debt_protocol_1.value.amount);

    console.log(balance_collateral_payer_2.value.amount);
    console.log(balance_debt_payer_2.value.amount);
    console.log(balance_collateral_protocol_2.value.amount);
    console.log(balance_debt_protocol_2.value.amount);
  });
});
