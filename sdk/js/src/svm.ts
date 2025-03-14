import {
  Connection,
  Keypair,
  PublicKey,
  Transaction,
  TransactionInstruction,
  SystemProgram,
} from "@solana/web3.js";
import * as anchor from "@coral-xyz/anchor";
import {
  BidSvmOnChain,
  BidSvmSwap,
  ExpressRelaySvmConfig,
  OpportunitySvmSwap,
  TokenAccountInitializationConfig,
  TokenAccountInitializationConfigs,
} from "./types";
import {
  ASSOCIATED_TOKEN_PROGRAM_ID,
  createAssociatedTokenAccountIdempotentInstruction,
  createCloseAccountInstruction,
  createSyncNativeInstruction,
  getAssociatedTokenAddressSync,
  NATIVE_MINT,
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

export const FEE_SPLIT_PRECISION = new anchor.BN(10000);

export function getConfigRouterPda(
  chain: string,
  router: PublicKey,
): PublicKey {
  const expressRelayProgram = getExpressRelayProgram(chain);

  return PublicKey.findProgramAddressSync(
    [Buffer.from("config_router"), router.toBuffer()],
    expressRelayProgram,
  )[0];
}

export function getExpressRelayMetadataPda(chain: string): PublicKey {
  const expressRelayProgram = getExpressRelayProgram(chain);

  return PublicKey.findProgramAddressSync(
    [Buffer.from("metadata")],
    expressRelayProgram,
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
  feeReceiverRelayer: PublicKey,
): Promise<TransactionInstruction> {
  const expressRelay = new Program<ExpressRelay>(
    expressRelayIdl as ExpressRelay,
    {} as AnchorProvider,
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
  tokenProgram: PublicKey,
): PublicKey {
  return getAssociatedTokenAddressSync(
    tokenMintAddress,
    owner,
    true, //allow owner to be off-curve
    tokenProgram,
    ASSOCIATED_TOKEN_PROGRAM_ID,
  );
}

export function createAtaIdempotentInstruction(
  owner: PublicKey,
  mint: PublicKey,
  payer: PublicKey = owner,
  tokenProgram: PublicKey,
): [PublicKey, TransactionInstruction] {
  const ataAddress = getAssociatedTokenAddress(owner, mint, tokenProgram);
  const createUserTokenAccountIx =
    createAssociatedTokenAccountIdempotentInstruction(
      payer,
      ataAddress,
      owner,
      mint,
      tokenProgram,
      ASSOCIATED_TOKEN_PROGRAM_ID,
    );
  return [ataAddress, createUserTokenAccountIx];
}

export async function constructSwapInstruction(
  searcher: PublicKey,
  swapOpportunity: OpportunitySvmSwap,
  bidAmount: anchor.BN,
  deadline: anchor.BN,
  chainId: string,
  feeReceiverRelayer: PublicKey,
  relayerSigner: PublicKey,
): Promise<TransactionInstruction> {
  const expressRelay = new Program<ExpressRelay>(
    expressRelayIdl as ExpressRelay,
    {} as AnchorProvider,
  );
  const expressRelayMetadata = getExpressRelayMetadataPda(chainId);
  const svmConstants = SVM_CONSTANTS[chainId];

  const {
    searcherToken,
    tokenProgramSearcher,
    tokenProgramUser,
    userToken,
    user,
    mintFee,
    feeTokenProgram,
    router,
  } = extractSwapInfo(swapOpportunity);

  if (
    swapOpportunity.tokens.type === "searcher_specified" &&
    swapOpportunity.feeToken === "user_token"
  ) {
    // scale bid amount by FEE_SPLIT_PRECISION/(FEE_SPLIT_PRECISION-fees) to account for fees
    const denominator = FEE_SPLIT_PRECISION.sub(
      new anchor.BN(
        swapOpportunity.platformFeeBps + swapOpportunity.referralFeeBps,
      ),
    );
    const numerator = bidAmount.mul(FEE_SPLIT_PRECISION);
    // add denominator - 1 to round up
    bidAmount = numerator
      .add(denominator.sub(new anchor.BN(1)))
      .div(denominator);
  }

  const swapArgs = {
    amountSearcher:
      swapOpportunity.tokens.type === "searcher_specified"
        ? new anchor.BN(swapOpportunity.tokens.searcherAmount)
        : bidAmount,
    amountUser:
      swapOpportunity.tokens.type === "user_specified"
        ? new anchor.BN(swapOpportunity.tokens.userTokenAmountIncludingFees)
        : bidAmount,
    referralFeeBps: new anchor.BN(swapOpportunity.referralFeeBps),
    deadline,
    feeToken:
      swapOpportunity.feeToken === "searcher_token"
        ? { searcher: {} }
        : { user: {} },
  };
  const ixSwap = await expressRelay.methods
    .swap(swapArgs)
    .accountsStrict({
      expressRelayMetadata,
      searcher,
      user: swapOpportunity.userWalletAddress,
      searcherTaMintSearcher: getAssociatedTokenAddress(
        searcher,
        searcherToken,
        tokenProgramSearcher,
      ),
      searcherTaMintUser: getAssociatedTokenAddress(
        searcher,
        userToken,
        tokenProgramUser,
      ),
      userAtaMintSearcher: getAssociatedTokenAddress(
        user,
        searcherToken,
        tokenProgramSearcher,
      ),
      tokenProgramSearcher: tokenProgramSearcher,
      mintSearcher: searcherToken,
      userAtaMintUser: getAssociatedTokenAddress(
        user,
        userToken,
        tokenProgramUser,
      ),
      tokenProgramUser: tokenProgramUser,
      mintUser: userToken,
      routerFeeReceiverTa: getAssociatedTokenAddress(
        router,
        mintFee,
        feeTokenProgram,
      ),
      relayerFeeReceiverAta: getAssociatedTokenAddress(
        feeReceiverRelayer,
        mintFee,
        feeTokenProgram,
      ),
      tokenProgramFee: feeTokenProgram,
      mintFee,
      expressRelayFeeReceiverAta: getAssociatedTokenAddress(
        expressRelayMetadata,
        mintFee,
        feeTokenProgram,
      ),
      relayerSigner,
    })
    .instruction();
  ixSwap.programId = svmConstants.expressRelayProgram;
  return ixSwap;
}

function extractSwapInfo(swapOpportunity: OpportunitySvmSwap): {
  tokenProgramUser: PublicKey;
  userToken: PublicKey;
  user: PublicKey;
  mintFee: PublicKey;
  feeTokenProgram: PublicKey;
  router: PublicKey;
  searcherToken: PublicKey;
  tokenProgramSearcher: PublicKey;
  tokenInitializationConfigs: TokenAccountInitializationConfigs;
} {
  const tokenProgramSearcher = swapOpportunity.tokens.tokenProgramSearcher;
  const tokenProgramUser = swapOpportunity.tokens.tokenProgramUser;
  const searcherToken = swapOpportunity.tokens.searcherToken;
  const userToken = swapOpportunity.tokens.userToken;
  const user = swapOpportunity.userWalletAddress;
  const [mintFee, feeTokenProgram] =
    swapOpportunity.feeToken === "searcher_token"
      ? [searcherToken, tokenProgramSearcher]
      : [userToken, tokenProgramUser];
  const router = swapOpportunity.routerAccount;
  const tokenInitializationConfigs = swapOpportunity.tokenInitializationConfigs;
  return {
    searcherToken,
    tokenProgramSearcher,
    tokenProgramUser,
    userToken,
    user,
    mintFee,
    feeTokenProgram,
    router,
    tokenInitializationConfigs,
  };
}

type TokenAccountToCreate = {
  payer: PublicKey;
  owner: PublicKey;
  mint: PublicKey;
  program: PublicKey;
};

type TokenAccountInitializationParams = {
  owner: PublicKey;
  mint: PublicKey;
  program: PublicKey;
  config: TokenAccountInitializationConfig;
};

function getTokenAccountToCreate(
  searcher: PublicKey,
  user: PublicKey,
  params: TokenAccountInitializationParams,
): TokenAccountToCreate | undefined {
  if (params.config === "searcher_payer") {
    return {
      payer: searcher,
      owner: params.owner,
      mint: params.mint,
      program: params.program,
    };
  } else if (params.config === "user_payer") {
    return {
      payer: user,
      owner: params.owner,
      mint: params.mint,
      program: params.program,
    };
  } else if (params.config === "unneeded") {
    return undefined;
  }
}

function getTokenAccountsToCreate(
  {
    searcher,
    user,
    router,
    feeReceiverRelayer,
    expressRelayMetadata,
    mintSearcher,
    tokenProgramSearcher,
    mintFee,
    feeTokenProgram,
  }: {
    searcher: PublicKey;
    user: PublicKey;
    router: PublicKey;
    feeReceiverRelayer: PublicKey;
    expressRelayMetadata: PublicKey;
    mintSearcher: PublicKey;
    tokenProgramSearcher: PublicKey;
    mintFee: PublicKey;
    feeTokenProgram: PublicKey;
  },
  tokenInitializationConfigs: TokenAccountInitializationConfigs,
): TokenAccountToCreate[] {
  const tokenAccountInitializationParams: TokenAccountInitializationParams[] = [
    {
      config: tokenInitializationConfigs.userAtaMintSearcher,
      owner: user,
      mint: mintSearcher,
      program: tokenProgramSearcher,
    },
    {
      config: tokenInitializationConfigs.userAtaMintUser,
      owner: user,
      mint: NATIVE_MINT,
      program: TOKEN_PROGRAM_ID,
    },
    {
      config: tokenInitializationConfigs.routerFeeReceiverAta,
      owner: router,
      mint: mintFee,
      program: feeTokenProgram,
    },
    {
      config: tokenInitializationConfigs.relayerFeeReceiverAta,
      owner: feeReceiverRelayer,
      mint: mintFee,
      program: feeTokenProgram,
    },
    {
      config: tokenInitializationConfigs.expressRelayFeeReceiverAta,
      owner: expressRelayMetadata,
      mint: mintFee,
      program: feeTokenProgram,
    },
  ];

  return tokenAccountInitializationParams
    .map((params) => getTokenAccountToCreate(searcher, user, params))
    .filter((account) => account !== undefined);
}

export function getWrapSolInstructions(
  payer: PublicKey,
  owner: PublicKey,
  amount: anchor.BN,
  createAta: boolean = true,
): TransactionInstruction[] {
  const instructions = [];
  const [ata, instruction] = createAtaIdempotentInstruction(
    owner,
    NATIVE_MINT,
    payer,
    TOKEN_PROGRAM_ID,
  );
  if (createAta) {
    instructions.push(instruction);
  }
  instructions.push(
    SystemProgram.transfer({
      fromPubkey: owner,
      toPubkey: ata,
      lamports: BigInt(amount.toString()),
    }),
  );
  instructions.push(createSyncNativeInstruction(ata, TOKEN_PROGRAM_ID));
  return instructions;
}

export function getUnwrapSolInstruction(
  owner: PublicKey,
): TransactionInstruction {
  const ata = getAssociatedTokenAddress(owner, NATIVE_MINT, TOKEN_PROGRAM_ID);
  return createCloseAccountInstruction(ata, owner, owner);
}

export async function constructSwapBid(
  tx: Transaction,
  searcher: PublicKey,
  swapOpportunity: OpportunitySvmSwap,
  bidAmount: anchor.BN,
  deadline: anchor.BN,
  chainId: string,
  feeReceiverRelayer: PublicKey,
  relayerSigner: PublicKey,
): Promise<BidSvmSwap> {
  const expressRelayMetadata = getExpressRelayMetadataPda(chainId);
  const {
    tokenProgramSearcher,
    userToken,
    searcherToken,
    user,
    mintFee,
    feeTokenProgram,
    router,
    tokenInitializationConfigs,
  } = extractSwapInfo(swapOpportunity);

  const tokenAccountsToCreate = getTokenAccountsToCreate(
    {
      searcher,
      user,
      router,
      feeReceiverRelayer,
      expressRelayMetadata,
      mintSearcher: searcherToken,
      tokenProgramSearcher,
      mintFee,
      feeTokenProgram,
    },
    tokenInitializationConfigs,
  );

  for (const account of tokenAccountsToCreate) {
    tx.instructions.push(
      createAtaIdempotentInstruction(
        account.owner,
        account.mint,
        account.payer,
        account.program,
      )[1],
    );
  }
  if (userToken.equals(NATIVE_MINT)) {
    if (swapOpportunity.tokens.type === "searcher_specified") {
      tx.instructions.push(
        ...getWrapSolInstructions(searcher, user, bidAmount, false), // this account creation is handled in the ata initialization section
      );
    } else {
      tx.instructions.push(
        ...getWrapSolInstructions(
          searcher,
          user,
          swapOpportunity.tokens.userTokenAmountIncludingFees,
        ),
      );
    }
  }
  const swapInstruction = await constructSwapInstruction(
    searcher,
    swapOpportunity,
    bidAmount,
    deadline,
    chainId,
    feeReceiverRelayer,
    relayerSigner,
  );
  tx.instructions.push(swapInstruction);
  if (searcherToken.equals(NATIVE_MINT)) {
    tx.instructions.push(getUnwrapSolInstruction(user));
  }
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
  feeReceiverRelayer: PublicKey,
): Promise<BidSvmOnChain> {
  const ixSubmitBid = await constructSubmitBidInstruction(
    searcher,
    router,
    permissionKey,
    bidAmount,
    deadline,
    chainId,
    relayerSigner,
    feeReceiverRelayer,
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
  connection: Connection,
): Promise<ExpressRelaySvmConfig> {
  const provider = new AnchorProvider(
    connection,
    new NodeWallet(new Keypair()),
  );
  const expressRelay = new Program<ExpressRelay>(
    expressRelayIdl as ExpressRelay,
    provider,
  );
  const metadata = await expressRelay.account.expressRelayMetadata.fetch(
    getExpressRelayMetadataPda(chainId),
  );
  return {
    feeReceiverRelayer: metadata.feeReceiverRelayer,
    relayerSigner: metadata.relayerSigner,
  };
}
