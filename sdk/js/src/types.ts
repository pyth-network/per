import type { components } from "./serverTypes";
import { PublicKey, Transaction } from "@solana/web3.js";
import { OrderStateAndAddress } from "@kamino-finance/limo-sdk/dist/utils";
import { VersionedTransaction } from "@solana/web3.js";
import * as anchor from "@coral-xyz/anchor";

/**
 * SVM token with contract address and amount
 */
export type TokenAmountSvm = {
  token: PublicKey;
  amount: bigint;
};

export type BidId = string;
export type ChainId = string;
/**
 * Bid parameters
 */
export type BidParams = {
  /**
   * Bid amount in wei
   */
  amount: bigint;
  /**
   * Bid nonce, used to prevent replay of a submitted signature.
   * This can be set to a random uint256 when creating a new signature
   */
  nonce: bigint;
  /**
   * Unix timestamp for when the bid is no longer valid in seconds
   */
  deadline: bigint;
};

export type OpportunitySvmMetadata = {
  /**
   * The chain id where the opportunity will be executed.
   */
  chainId: ChainId;
  /**
   * Unique identifier for the opportunity
   */
  opportunityId: string;
};

export type OpportunitySvmLimo = {
  order: OrderStateAndAddress;
  program: "limo";
  slot: number;
} & OpportunitySvmMetadata;

export type SvmSwapTokens = (
  | {
      userAmount: bigint;
      userTokenAmountIncludingFees: bigint;
      type: "user_specified";
    }
  | {
      searcherAmount: bigint;
      type: "searcher_specified";
    }
) & {
  tokenProgramSearcher: PublicKey;
  tokenProgramUser: PublicKey;
  searcherToken: PublicKey;
  userToken: PublicKey;
};

export type TokenAccountInitializationConfig =
  components["schemas"]["TokenAccountInitializationConfig"];

export type TokenAccountInitializationConfigs = Record<
  | "expressRelayFeeReceiverAta"
  | "relayerFeeReceiverAta"
  | "routerFeeReceiverAta"
  | "userAtaMintSearcher"
  | "userAtaMintUser",
  TokenAccountInitializationConfig
>;

export type OpportunitySvmSwap = {
  permissionAccount: PublicKey;
  routerAccount: PublicKey;
  userWalletAddress: PublicKey;
  userMintUserBalance: anchor.BN;
  feeToken: "searcher_token" | "user_token";
  referralFeeBps: number;
  platformFeeBps: number;
  tokens: SvmSwapTokens;
  program: "swap";
  tokenInitializationConfigs: TokenAccountInitializationConfigs;
  memo?: string;
  cancellable: boolean;
  minimumDeadline: number;
} & OpportunitySvmMetadata;

export type OpportunitySvm = OpportunitySvmLimo | OpportunitySvmSwap;

export type OpportunityCreate = Omit<OpportunitySvmLimo, "opportunityId">;

export type Opportunity = OpportunitySvm;
/**
 * All the parameters necessary to represent an opportunity
 */

export type Bid = BidSvm;
/**
 * Necessary accounts for submitting a SVM bid. These can be fetched from on-chain program data.
 */
export type ExpressRelaySvmConfig = {
  /**
   * @description The relayer signer account. All submitted transactions will be signed by this account.
   */
  relayerSigner: PublicKey;
  /**
   * @description The fee collection account for the relayer.
   */
  feeReceiverRelayer: PublicKey;
};

/**
 * Represents a raw SVM bid on acquiring a permission key
 */
export type BidSvmOnChain = {
  /**
   * @description Transaction object.
   * @example SGVsbG8sIFdvcmxkIQ
   */
  transaction: Transaction;
  /**
   * @description The chain id to bid on.
   * @example solana
   */
  chainId: ChainId;
  /**
   * @description The minimum slot required for the bid to be executed successfully
   * None if the bid can be executed at any recent slot
   * @example 293106477
   */
  slot?: number | null;
  type: "onchain";
  /**
   * @description The execution environment for the bid.
   */
  env: "svm";
};

/**
 * Represents a raw SVM bid to fulfill a swap opportunity
 */
export type BidSvmSwap = {
  /**
   * @description Transaction object.
   * @example SGVsbG8sIFdvcmxkIQ
   */
  transaction: Transaction;
  /**
   * @description The chain id to bid on.
   * @example solana
   */
  chainId: ChainId;
  /**
   * @description The id of the swap opportunity to bid on.
   * @example obo3ee3e-58cc-4372-a567-0e02b2c3d479
   */
  opportunityId: string;
  type: "swap";
  /**
   * @description The execution environment for the bid.
   */
  env: "svm";
};

export type BidSvm = BidSvmOnChain | BidSvmSwap;

export type BidStatusUpdate = {
  id: BidId;
} & components["schemas"]["BidStatus"];

export type BidStatusUpdateSvm = {
  id: BidId;
} & components["schemas"]["BidStatusSvm"];

export type BidResponse = components["schemas"]["Bid"];
export type BidResponseSvm = components["schemas"]["BidSvm"];

export type BidsResponse = {
  items: BidResponse[];
};

export type SvmConstantsConfig = {
  expressRelayProgram: PublicKey;
};

export type SvmChainUpdate = {
  chainId: ChainId;
  blockhash: string;
  latestPrioritizationFee: bigint;
};

export type OpportunityDeleteSvm = {
  chainId: ChainId;
  permissionAccount: PublicKey;
  program: components["schemas"]["ProgramSvm"];
  router: PublicKey;
};

export enum ChainType {
  SVM = "svm",
}

export type OpportunityDelete = OpportunityDeleteSvm & {
  chainType: ChainType.SVM;
};

export type SpecifiedTokenAmount = {
  side: "input" | "output";
  amount: number;
};

export type ReferralFeeInfo = {
  /**
   * @description The router account that referral fees will be sent to
   * @example 11111111111111111111111111111111
   */
  router: PublicKey;
  /**
   * @description The referral fee for the swap in bps
   * @example 10
   */
  referralFeeBps: number;
};

export type QuoteRequest = {
  chainId: ChainId;
  /**
   * @description The mint of the token that the user wants to swap from
   * @example So11111111111111111111111111111111111111112
   */
  inputTokenMint: PublicKey;
  /**
   * @description The mint of the token that the user wants to swap into
   * @example EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v
   */
  outputTokenMint: PublicKey;
  /**
   * @description Info about the referral fee. If not provided, no referral fee will be charged.
   */
  referralFeeInfo?: ReferralFeeInfo;
  /**
   * @description The specified token amount for the swap
   */
  specifiedTokenAmount: SpecifiedTokenAmount;
  /**
   * @description The user wallet account. If not provided, queries for an indicative price.
   * @example 11111111111111111111111111111111
   */
  userWallet?: PublicKey;
  /**
   * @description Optional memo to be included in the transaction.
   * @example "memo"
   */
  memo?: string;
  /**
   * @description Whether the quote is cancellable by the searcher between the time the quote is requested and the time the quote is signed and submitted back.
   * For cancellable quotes, the quote needs to be signed and submitted back to the API. If the quote is not cancellable, the user may broadcast the transaction to the blockchain on their own instead of submitting it back to the API.
   * Therefore cancellable quotes allow the integrator to reduce the number of API calls to one, but at the cost of potentially worse prices. Price-optimizing integrators should use the default value of false.
   * @default true
   */
  cancellable?: boolean;
  /**
   * @description Optional minimum transaction lifetime for the quotes in seconds.
   * @example 60
   */
  minimumLifetime?: number;
};

export type QuoteResponse = {
  chainId: ChainId;
  expirationTime?: Date;
  inputToken: TokenAmountSvm;
  outputToken: TokenAmountSvm;
  transaction?: VersionedTransaction;
  referenceId: string;
};

export type SubmitQuote = {
  chainId: ChainId;
  referenceId: components["schemas"]["SubmitQuote"]["reference_id"];
  userSignature: components["schemas"]["SubmitQuote"]["user_signature"];
};
