import { Address, Hex } from "viem";
import type { components } from "./serverTypes";
import { PublicKey, Transaction } from "@solana/web3.js";
import { OrderStateAndAddress } from "@kamino-finance/limo-sdk/dist/utils";
import { VersionedTransaction } from "@solana/web3.js";
import * as anchor from "@coral-xyz/anchor";

/**
 * ERC20 token with contract address and amount
 */
export type TokenAmount = {
  token: Address;
  amount: bigint;
};
/**
 * SVM token with contract address and amount
 */
export type TokenAmountSvm = {
  token: PublicKey;
  amount: bigint;
};

/**
 * TokenPermissions struct for permit2
 */
export type TokenPermissions = {
  token: Address;
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

export type OpportunityAdapterConfig = {
  /**
   * The chain id as a u64
   */
  chain_id: number;
  /**
   * The opportunity factory address
   */
  opportunity_adapter_factory: Address;
  /**
   * The hash of the bytecode used to initialize the opportunity adapter
   */
  opportunity_adapter_init_bytecode_hash: Hex;
  /**
   * The permit2 address
   */
  permit2: Address;
  /**
   * The weth address
   */
  weth: Address;
};
/**
 * Represents a valid opportunity ready to be executed
 */
export type OpportunityEvm = {
  /**
   * The chain id where the opportunity will be executed.
   */
  chainId: ChainId;

  /**
   * Permission key required for successful execution of the opportunity.
   */
  permissionKey: Hex;
  /**
   * Contract address to call for execution of the opportunity.
   */
  targetContract: Address;
  /**
   * Calldata for the targetContract call.
   */
  targetCalldata: Hex;
  /**
   * Value to send with the targetContract call.
   */
  targetCallValue: bigint;
  /**
   * Tokens required to execute the opportunity
   */
  sellTokens: TokenAmount[];
  /**
   * Tokens to receive after the opportunity is executed
   */
  buyTokens: TokenAmount[];
  /**
   * Unique identifier for the opportunity
   */
  opportunityId: string;
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
} & OpportunitySvmMetadata;

export type OpportunitySvm = OpportunitySvmLimo | OpportunitySvmSwap;

export type OpportunityCreate =
  | Omit<OpportunityEvm, "opportunityId">
  | Omit<OpportunitySvmLimo, "opportunityId">;

export type Opportunity = OpportunityEvm | OpportunitySvm;
/**
 * Represents a bid for an opportunity
 */
export type OpportunityBid = {
  /**
   * Opportunity unique identifier in uuid format
   */
  opportunityId: string;
  /**
   * The permission key required for successful execution of the opportunity.
   */
  permissionKey: Hex;
  /**
   * Executor address
   */
  executor: Address;
  /**
   * Signature of the executor
   */
  signature: Hex;

  bid: BidParams;
};
/**
 * All the parameters necessary to represent an opportunity
 */

export type Bid = BidEvm | BidSvm;
/**
 * Represents a raw EVM bid on acquiring a permission key
 */
export type BidEvm = {
  /**
   * The permission key to bid on
   * @example 0xc0ffeebabe
   *
   */
  permissionKey: Hex;
  /**
   * @description Amount of bid in wei.
   * @example 10
   */
  amount: bigint;
  /**
   * @description Calldata for the targetContract call.
   * @example 0xdeadbeef
   */
  targetCalldata: Hex;
  /**
   * @description The chain id to bid on.
   * @example sepolia
   */
  chainId: ChainId;
  /**
   * @description The targetContract address to call.
   * @example 0xcA11bde05977b3631167028862bE2a173976CA11
   */
  targetContract: Address;
  /**
   * @description The execution environment for the bid.
   */
  env: "evm";
};

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

export type BidStatusUpdateEvm = {
  id: BidId;
} & components["schemas"]["BidStatusEvm"];

export type BidResponse = components["schemas"]["Bid"];
export type BidResponseSvm = components["schemas"]["BidSvm"];
export type BidResponseEvm = components["schemas"]["BidEvm"];

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

export type OpportunityDeleteEvm = {
  chainId: ChainId;
  permissionKey: Hex;
};

export enum ChainType {
  EVM = "evm",
  SVM = "svm",
}

export type OpportunityDelete =
  | (OpportunityDeleteSvm & {
      chainType: ChainType.SVM;
    })
  | (OpportunityDeleteEvm & {
      chainType: ChainType.EVM;
    });

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
   * @description Optional minimum transaction lifetime in seconds.
   * @example 10
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
