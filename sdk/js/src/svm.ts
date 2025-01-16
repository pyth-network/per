import {
  Connection,
  Keypair,
  PublicKey,
  Transaction,
  TransactionInstruction,
} from "@solana/web3.js";
import * as anchor from "@coral-xyz/anchor";
import {
  BidSvmOnChain,
  BidSvmSwap,
  ExpressRelaySvmConfig,
  OpportunitySvmSwap,
} from "./types";
import {
  ASSOCIATED_TOKEN_PROGRAM_ID,
  createAssociatedTokenAccountIdempotentInstruction,
  getAssociatedTokenAddressSync,
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
    true, //allow owner to be off-curve
    tokenProgram,
    ASSOCIATED_TOKEN_PROGRAM_ID
  );
}

export function createAtaIdempotentInstruction(
  owner: PublicKey,
  mint: PublicKey,
  payer: PublicKey = owner,
  tokenProgram: PublicKey
): [PublicKey, TransactionInstruction] {
  const ataAddress = getAssociatedTokenAddress(owner, mint, tokenProgram);
  const createUserTokenAccountIx =
    createAssociatedTokenAccountIdempotentInstruction(
      payer,
      ataAddress,
      owner,
      mint,
      tokenProgram,
      ASSOCIATED_TOKEN_PROGRAM_ID
    );
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
): Promise<BidSvmSwap> {
  const expressRelay = new Program<ExpressRelay>(
    expressRelayIdl as ExpressRelay,
    {} as AnchorProvider
  );
  const expressRelayMetadata = getExpressRelayMetadataPda(chainId);
  const svmConstants = SVM_CONSTANTS[chainId];

  const inputTokenProgram = swapOpportunity.tokens.inputTokenProgram;
  const outputTokenProgram = swapOpportunity.tokens.outputTokenProgram;
  const inputToken =
    swapOpportunity.tokens.type === "input_specified"
      ? swapOpportunity.tokens.inputToken.token
      : swapOpportunity.tokens.inputToken;
  const outputToken =
    swapOpportunity.tokens.type === "output_specified"
      ? swapOpportunity.tokens.outputToken.token
      : swapOpportunity.tokens.outputToken;
  const trader = swapOpportunity.userWalletAddress;
  const [mintFee, feeTokenProgram] =
    swapOpportunity.feeToken === "input_token"
      ? [inputToken, inputTokenProgram]
      : [outputToken, outputTokenProgram];
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
    referralFeeBps: new anchor.BN(swapOpportunity.referralFeeBps),
    deadline,
    feeToken:
      swapOpportunity.feeToken === "input_token"
        ? { input: {} }
        : { output: {} },
  };
  const ixSwap = await expressRelay.methods
    .swap(swapArgs)
    .accountsStrict({
      expressRelayMetadata,
      searcher,
      trader: swapOpportunity.userWalletAddress,
      searcherInputTa: getAssociatedTokenAddress(
        searcher,
        inputToken,
        inputTokenProgram
      ),
      searcherOutputTa: getAssociatedTokenAddress(
        searcher,
        outputToken,
        outputTokenProgram
      ),
      traderInputAta: getAssociatedTokenAddress(
        trader,
        inputToken,
        inputTokenProgram
      ),
      tokenProgramInput: inputTokenProgram,
      mintInput: inputToken,
      traderOutputAta: getAssociatedTokenAddress(
        trader,
        outputToken,
        outputTokenProgram
      ),
      tokenProgramOutput: outputTokenProgram,
      mintOutput: outputToken,
      routerFeeReceiverTa: getAssociatedTokenAddress(
        router,
        mintFee,
        feeTokenProgram
      ),
      relayerFeeReceiverAta: getAssociatedTokenAddress(
        relayerSigner,
        mintFee,
        feeTokenProgram
      ),
      tokenProgramFee: feeTokenProgram,
      mintFee,
      expressRelayFeeReceiverAta: getAssociatedTokenAddress(
        expressRelayMetadata,
        mintFee,
        feeTokenProgram
      ),
    })
    .instruction();
  ixSwap.programId = svmConstants.expressRelayProgram;
  tx.instructions.push(
    createAtaIdempotentInstruction(
      router,
      mintFee,
      searcher,
      feeTokenProgram
    )[1]
  );
  tx.instructions.push(
    createAtaIdempotentInstruction(
      relayerSigner,
      mintFee,
      searcher,
      feeTokenProgram
    )[1]
  );
  tx.instructions.push(
    createAtaIdempotentInstruction(
      expressRelayMetadata,
      mintFee,
      searcher,
      feeTokenProgram
    )[1]
  );
  tx.instructions.push(
    createAtaIdempotentInstruction(
      trader,
      outputToken,
      searcher,
      outputTokenProgram
    )[1]
  );
  tx.instructions.push(ixSwap);

  return {
    transaction: tx,
    opportunityId: swapOpportunity.opportunityId,
    type: "swap",
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
): Promise<BidSvmOnChain> {
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
    type: "onchain",
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
