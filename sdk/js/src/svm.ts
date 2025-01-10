import {
  Connection,
  Keypair,
  PublicKey,
  Transaction,
  TransactionInstruction,
} from "@solana/web3.js";
import * as anchor from "@coral-xyz/anchor";
import { BidSvm, ExpressRelaySvmConfig, OpportunitySvmSwap } from "./types";
import {
  ASSOCIATED_TOKEN_PROGRAM_ID,
  createAssociatedTokenAccountInstruction,
  getAssociatedTokenAddressSync,
  TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import { AnchorProvider, Program } from "@coral-xyz/anchor";
import { ExpressRelay } from "./expressRelayTypes";
import expressRelayIdl from "./idl/idlExpressRelay.json";
import { SVM_CONSTANTS } from "./const";
import NodeWallet from "@coral-xyz/anchor/dist/cjs/nodewallet";

function getExpressRelayProgram(chain: string): PublicKey {
  if (!SVM_CONSTANTS[chain]) {
    throw new Error(`Chain ${chain} not supported`);
  }
  return SVM_CONSTANTS[chain].expressRelayProgram;
}

export function getConfigRouterPda(
  chain: string,
  router: PublicKey
): PublicKey {
  const expressRelayProgram = getExpressRelayProgram(chain);

  return PublicKey.findProgramAddressSync(
    [Buffer.from("config_router"), router.toBuffer()],
    expressRelayProgram
  )[0];
}

export function getExpressRelayMetadataPda(chain: string): PublicKey {
  const expressRelayProgram = getExpressRelayProgram(chain);

  return PublicKey.findProgramAddressSync(
    [Buffer.from("metadata")],
    expressRelayProgram
  )[0];
}

export async function constructSubmitBidInstruction(
  searcher: PublicKey,
  router: PublicKey,
  permissionKey: PublicKey,
  bidAmount: anchor.BN,
  deadline: anchor.BN,
  chainId: string,
  relayerSigner: PublicKey,
  feeReceiverRelayer: PublicKey
): Promise<TransactionInstruction> {
  const expressRelay = new Program<ExpressRelay>(
    expressRelayIdl as ExpressRelay,
    {} as AnchorProvider
  );

  const configRouter = getConfigRouterPda(chainId, router);
  const expressRelayMetadata = getExpressRelayMetadataPda(chainId);
  const svmConstants = SVM_CONSTANTS[chainId];

  const ixSubmitBid = await expressRelay.methods
    .submitBid({
      deadline,
      bidAmount,
    })
    .accountsStrict({
      searcher,
      relayerSigner,
      permission: permissionKey,
      router,
      configRouter,
      expressRelayMetadata,
      feeReceiverRelayer,
      systemProgram: anchor.web3.SystemProgram.programId,
      sysvarInstructions: anchor.web3.SYSVAR_INSTRUCTIONS_PUBKEY,
    })
    .instruction();
  ixSubmitBid.programId = svmConstants.expressRelayProgram;

  return ixSubmitBid;
}

export function getAssociatedTokenAddress(
  owner: PublicKey,
  tokenMintAddress: PublicKey,
  tokenProgram: PublicKey
): PublicKey {
  return getAssociatedTokenAddressSync(
    tokenMintAddress,
    owner,
    true,
    tokenProgram,
    ASSOCIATED_TOKEN_PROGRAM_ID
  );
}

export function createAssociatedTokenAccountIdempotentInstruction(
  owner: PublicKey,
  mint: PublicKey,
  payer: PublicKey = owner,
  tokenProgram: PublicKey
): [PublicKey, TransactionInstruction] {
  const ataAddress = getAssociatedTokenAddress(owner, mint, tokenProgram);
  const createUserTokenAccountIx = createAssociatedTokenAccountInstruction(
    payer,
    ataAddress,
    owner,
    mint,
    tokenProgram,
    ASSOCIATED_TOKEN_PROGRAM_ID
  );
  // idempotent ix discriminator is 1
  createUserTokenAccountIx.data = Buffer.from([1]);
  return [ataAddress, createUserTokenAccountIx];
}

export async function constructSwapBid(
  tx: Transaction,
  searcher: PublicKey,
  swapOpportunity: OpportunitySvmSwap,
  bidAmount: anchor.BN,
  deadline: anchor.BN,
  chainId: string,
  relayerSigner: PublicKey
): Promise<BidSvm> {
  const expressRelay = new Program<ExpressRelay>(
    expressRelayIdl as ExpressRelay,
    {} as AnchorProvider
  );
  const expressRelayMetadata = getExpressRelayMetadataPda(chainId);
  const svmConstants = SVM_CONSTANTS[chainId];

  const tokenProgramInput = TOKEN_PROGRAM_ID;
  const tokenProgramOutput = TOKEN_PROGRAM_ID;
  const tokenProgramFee = TOKEN_PROGRAM_ID;
  const mintInput =
    swapOpportunity.tokens.type === "input_specified"
      ? swapOpportunity.tokens.inputToken.token
      : swapOpportunity.tokens.inputToken;
  const mintOutput =
    swapOpportunity.tokens.type === "output_specified"
      ? swapOpportunity.tokens.outputToken.token
      : swapOpportunity.tokens.outputToken;
  const trader = swapOpportunity.userWalletAddress;
  const mintFee = mintInput;
  const router = swapOpportunity.routerAccount;

  const swapArgs = {
    amountInput:
      swapOpportunity.tokens.type === "input_specified"
        ? new anchor.BN(swapOpportunity.tokens.inputToken.amount)
        : bidAmount,
    amountOutput:
      swapOpportunity.tokens.type === "output_specified"
        ? new anchor.BN(swapOpportunity.tokens.outputToken.amount)
        : bidAmount,
    referralFeeBps: new anchor.BN(0),
    deadline,
    feeToken: { input: {} },
  };
  const ixSwap = await expressRelay.methods
    .swap(swapArgs)
    .accountsStrict({
      expressRelayMetadata,
      searcher,
      trader: swapOpportunity.userWalletAddress,
      searcherInputTa: getAssociatedTokenAddress(
        searcher,
        mintInput,
        tokenProgramInput
      ),
      searcherOutputTa: getAssociatedTokenAddress(
        searcher,
        mintOutput,
        tokenProgramOutput
      ),
      traderInputAta: getAssociatedTokenAddress(
        trader,
        mintInput,
        tokenProgramInput
      ),
      tokenProgramInput,
      mintInput,
      traderOutputAta: getAssociatedTokenAddress(
        trader,
        mintOutput,
        tokenProgramOutput
      ),
      tokenProgramOutput,
      mintOutput,
      routerFeeReceiverTa: getAssociatedTokenAddress(
        router,
        mintFee,
        tokenProgramFee
      ),
      relayerFeeReceiverAta: getAssociatedTokenAddress(
        relayerSigner,
        mintFee,
        tokenProgramFee
      ),
      tokenProgramFee,
      mintFee,
      expressRelayFeeReceiverAta: getAssociatedTokenAddress(
        expressRelayMetadata,
        mintFee,
        tokenProgramFee
      ),
    })
    .instruction();
  ixSwap.programId = svmConstants.expressRelayProgram;
  tx.instructions.push(
    createAssociatedTokenAccountIdempotentInstruction(
      router,
      mintFee,
      searcher,
      tokenProgramFee
    )[1]
  );
  tx.instructions.push(
    createAssociatedTokenAccountIdempotentInstruction(
      relayerSigner,
      mintFee,
      searcher,
      tokenProgramFee
    )[1]
  );
  tx.instructions.push(
    createAssociatedTokenAccountIdempotentInstruction(
      expressRelayMetadata,
      mintFee,
      searcher,
      tokenProgramFee
    )[1]
  );
  tx.instructions.push(
    createAssociatedTokenAccountIdempotentInstruction(
      trader,
      mintOutput,
      searcher,
      tokenProgramOutput
    )[1]
  );
  tx.instructions.push(ixSwap);

  return {
    transaction: tx,
    chainId: chainId,
    env: "svm",
  };
}

export async function constructSvmBid(
  tx: Transaction,
  searcher: PublicKey,
  router: PublicKey,
  permissionKey: PublicKey,
  bidAmount: anchor.BN,
  deadline: anchor.BN,
  chainId: string,
  relayerSigner: PublicKey,
  feeReceiverRelayer: PublicKey
): Promise<BidSvm> {
  const ixSubmitBid = await constructSubmitBidInstruction(
    searcher,
    router,
    permissionKey,
    bidAmount,
    deadline,
    chainId,
    relayerSigner,
    feeReceiverRelayer
  );

  tx.instructions.unshift(ixSubmitBid);

  return {
    transaction: tx,
    chainId: chainId,
    env: "svm",
  };
}

export async function getExpressRelaySvmConfig(
  chainId: string,
  connection: Connection
): Promise<ExpressRelaySvmConfig> {
  const provider = new AnchorProvider(
    connection,
    new NodeWallet(new Keypair())
  );
  const expressRelay = new Program<ExpressRelay>(
    expressRelayIdl as ExpressRelay,
    provider
  );
  const metadata = await expressRelay.account.expressRelayMetadata.fetch(
    getExpressRelayMetadataPda(chainId)
  );
  return {
    feeReceiverRelayer: metadata.feeReceiverRelayer,
    relayerSigner: metadata.relayerSigner,
  };
}
