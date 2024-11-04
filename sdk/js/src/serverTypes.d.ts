/**
 * This file was auto-generated by openapi-typescript.
 * Do not make direct changes to the file.
 */

export interface paths {
  "/v1/bids": {
    /**
     * Returns at most 20 bids which were submitted after a specific time.
     * @description If no time is provided, the server will return the first bids.
     */
    get: operations["get_bids_by_time"];
    /**
     * Bid on a specific permission key for a specific chain.
     * @description Your bid will be verified by the server. Depending on the outcome of the auction, a transaction
     * containing your bid will be sent to the blockchain expecting the bid amount to be paid in the transaction.
     */
    post: operations["bid"];
  };
  "/v1/bids/{bid_id}": {
    /** Query the status of a specific bid. */
    get: operations["bid_status"];
  };
  "/v1/opportunities": {
    /**
     * Fetch opportunities ready for execution or historical opportunities
     * @description depending on the mode. You need to provide `chain_id` for historical mode.
     * Opportunities are sorted by creation time in ascending order.
     * Total number of opportunities returned is limited by 20.
     */
    get: operations["get_opportunities"];
    /**
     * Submit an opportunity ready to be executed.
     * @description The opportunity will be verified by the server. If the opportunity is valid, it will be stored in the database
     * and will be available for bidding.
     */
    post: operations["post_opportunity"];
    /** Delete all opportunities for specified data. */
    delete: operations["delete_opportunities"];
  };
  "/v1/opportunities/quote": {
    /**
     * Submit a quote request.
     * @description The server will estimate the quote price, which will be used to create an opportunity.
     * After a certain time, searcher bids are collected, the winning signed bid will be returned along with the estimated price.
     */
    post: operations["post_quote"];
  };
  "/v1/opportunities/{opportunity_id}/bids": {
    /** Bid on opportunity. */
    post: operations["opportunity_bid"];
  };
  "/v1/profiles/access_tokens": {
    /**
     * Revoke the authenticated profile access token.
     * @description Returns empty response.
     */
    delete: operations["delete_profile_access_token"];
  };
}

export type webhooks = Record<string, never>;

export interface components {
  schemas: {
    APIResponse: components["schemas"]["BidResult"];
    Bid: components["schemas"]["BidEvm"] | components["schemas"]["BidSvm"];
    BidEvm: {
      /**
       * @description Amount of bid in wei.
       * @example 10
       */
      amount: string;
      /**
       * @description The chain id to bid on.
       * @example op_sepolia
       */
      chain_id: string;
      /**
       * @description The permission key to bid on.
       * @example 0xdeadbeef
       */
      permission_key: string;
      /**
       * @description Calldata for the contract call.
       * @example 0xdeadbeef
       */
      target_calldata: string;
      /**
       * @description The contract address to call.
       * @example 0xcA11bde05977b3631167028862bE2a173976CA11
       */
      target_contract: string;
    };
    BidResult: {
      /**
       * @description The unique id created to identify the bid. This id can be used to query the status of the bid.
       * @example beedbeed-58cc-4372-a567-0e02b2c3d479
       */
      id: string;
      /**
       * @description The status of the request. If the bid was placed successfully, the status will be "OK".
       * @example OK
       */
      status: string;
    };
    BidStatus:
      | components["schemas"]["BidStatusEvm"]
      | components["schemas"]["BidStatusSvm"];
    BidStatusEvm:
      | {
          /** @enum {string} */
          type: "pending";
        }
      | {
          /**
           * Format: int32
           * @example 1
           */
          index: number;
          /** @example 0x103d4fbd777a36311b5161f2062490f761f25b67406badb2bace62bb170aa4e3 */
          result: string;
          /** @enum {string} */
          type: "submitted";
        }
      | {
          /**
           * Format: int32
           * @example 1
           */
          index?: number | null;
          /** @example 0x103d4fbd777a36311b5161f2062490f761f25b67406badb2bace62bb170aa4e3 */
          result?: string | null;
          /** @enum {string} */
          type: "lost";
        }
      | {
          /**
           * Format: int32
           * @example 1
           */
          index: number;
          /** @example 0x103d4fbd777a36311b5161f2062490f761f25b67406badb2bace62bb170aa4e3 */
          result: string;
          /** @enum {string} */
          type: "won";
        };
    BidStatusSvm:
      | {
          /** @enum {string} */
          type: "pending";
        }
      | {
          /** @example Jb2urXPyEh4xiBgzYvwEFe4q1iMxG1DNxWGGQg94AmKgqFTwLAiTiHrYiYxwHUB4DV8u5ahNEVtMMDm3sNSRdTg */
          result: string;
          /** @enum {string} */
          type: "submitted";
        }
      | {
          /** @example Jb2urXPyEh4xiBgzYvwEFe4q1iMxG1DNxWGGQg94AmKgqFTwLAiTiHrYiYxwHUB4DV8u5ahNEVtMMDm3sNSRdTg */
          result?: string | null;
          /** @enum {string} */
          type: "lost";
        }
      | {
          /** @example Jb2urXPyEh4xiBgzYvwEFe4q1iMxG1DNxWGGQg94AmKgqFTwLAiTiHrYiYxwHUB4DV8u5ahNEVtMMDm3sNSRdTg */
          result: string;
          /** @enum {string} */
          type: "won";
        }
      | {
          /** @example Jb2urXPyEh4xiBgzYvwEFe4q1iMxG1DNxWGGQg94AmKgqFTwLAiTiHrYiYxwHUB4DV8u5ahNEVtMMDm3sNSRdTg */
          result: string;
          /** @enum {string} */
          type: "expired";
        };
    BidStatusWithId: {
      bid_status: components["schemas"]["BidStatus"];
      id: string;
    };
    BidSvm: {
      /**
       * @description The chain id to bid on.
       * @example solana
       */
      chain_id: string;
      /**
       * @description The transaction for bid.
       * @example SGVsbG8sIFdvcmxkIQ==
       */
      transaction: string;
    };
    ClientMessage:
      | {
          /** @enum {string} */
          method: "subscribe";
          params: {
            chain_ids: string[];
          };
        }
      | {
          /** @enum {string} */
          method: "unsubscribe";
          params: {
            chain_ids: string[];
          };
        }
      | {
          /** @enum {string} */
          method: "post_bid";
          params: {
            bid: components["schemas"]["Bid"];
          };
        }
      | {
          /** @enum {string} */
          method: "post_opportunity_bid";
          params: {
            opportunity_bid: components["schemas"]["OpportunityBidEvm"];
            opportunity_id: string;
          };
        };
    ClientRequest: components["schemas"]["ClientMessage"] & {
      id: string;
    };
    ErrorBodyResponse: {
      error: string;
    };
    Opportunity:
      | components["schemas"]["OpportunityEvm"]
      | components["schemas"]["OpportunitySvm"];
    OpportunityBidEvm: {
      /**
       * @description The bid amount in wei.
       * @example 1000000000000000000
       */
      amount: string;
      /**
       * @description The latest unix timestamp in seconds until which the bid is valid.
       * @example 1000000000000000000
       */
      deadline: string;
      /**
       * @description The executor address.
       * @example 0x5FbDB2315678afecb367f032d93F642f64180aa2
       */
      executor: string;
      /**
       * @description The nonce of the bid permit signature.
       * @example 123
       */
      nonce: string;
      /**
       * @description The opportunity permission key.
       * @example 0xdeadbeefcafe
       */
      permission_key: string;
      /** @example 0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef12 */
      signature: string;
    };
    OpportunityBidResult: {
      /**
       * @description The unique id created to identify the bid. This id can be used to query the status of the bid.
       * @example beedbeed-58cc-4372-a567-0e02b2c3d479
       */
      id: string;
      /** @example OK */
      status: string;
    };
    /** @description The input type for creating a new opportunity. */
    OpportunityCreate:
      | components["schemas"]["OpportunityCreateEvm"]
      | components["schemas"]["OpportunityCreateSvm"];
    OpportunityCreateEvm: components["schemas"]["OpportunityCreateV1Evm"] & {
      /** @enum {string} */
      version: "v1";
    };
    /** @description Program specific parameters for the opportunity. */
    OpportunityCreateProgramParamsV1Svm:
      | {
          /**
           * @description The Limo order to be executed, encoded in base64.
           * @example DUcTi3rDyS5QEmZ4BNRBejtArmDCWaPYGfN44vBJXKL5
           */
          order: string;
          /**
           * @description Address of the order account.
           * @example DUcTi3rDyS5QEmZ4BNRBejtArmDCWaPYGfN44vBJXKL5
           */
          order_address: string;
          /** @enum {string} */
          program: "limo";
        }
      | {
          /**
           * Format: double
           * @description The maximum slippage percentage that the user is willing to accept.
           * @example 0.5
           */
          maximum_slippage_percentage: number;
          /** @enum {string} */
          program: "phantom";
          /**
           * @description The user wallet address which requested the quote from the wallet.
           * @example DUcTi3rDyS5QEmZ4BNRBejtArmDCWaPYGfN44vBJXKL5
           */
          user_wallet_address: string;
        };
    OpportunityCreateSvm: components["schemas"]["OpportunityCreateV1Svm"] & {
      /** @enum {string} */
      version: "v1";
    };
    /**
     * @description Opportunity parameters needed for on-chain execution.
     * If a searcher signs the opportunity and have approved enough tokens to opportunity adapter,
     * by calling this target contract with the given target calldata and structures, they will
     * send the tokens specified in the `sell_tokens` field and receive the tokens specified in the `buy_tokens` field.
     */
    OpportunityCreateV1Evm: {
      buy_tokens: components["schemas"]["TokenAmountEvm"][];
      /**
       * @description The chain id where the opportunity will be executed.
       * @example op_sepolia
       */
      chain_id: string;
      /**
       * @description The permission key required for successful execution of the opportunity.
       * @example 0xdeadbeefcafe
       */
      permission_key: string;
      sell_tokens: components["schemas"]["TokenAmountEvm"][];
      /**
       * @description The value to send with the contract call.
       * @example 1
       */
      target_call_value: string;
      /**
       * @description Calldata for the target contract call.
       * @example 0xdeadbeef
       */
      target_calldata: string;
      /**
       * @description The contract address to call for execution of the opportunity.
       * @example 0xcA11bde05977b3631167028862bE2a173976CA11
       */
      target_contract: string;
    };
    /**
     * @description Opportunity parameters needed for on-chain execution.
     * Parameters may differ for each program.
     */
    OpportunityCreateV1Svm: (
      | {
          /**
           * @description The Limo order to be executed, encoded in base64.
           * @example DUcTi3rDyS5QEmZ4BNRBejtArmDCWaPYGfN44vBJXKL5
           */
          order: string;
          /**
           * @description Address of the order account.
           * @example DUcTi3rDyS5QEmZ4BNRBejtArmDCWaPYGfN44vBJXKL5
           */
          order_address: string;
          /** @enum {string} */
          program: "limo";
        }
      | {
          /**
           * Format: double
           * @description The maximum slippage percentage that the user is willing to accept.
           * @example 0.5
           */
          maximum_slippage_percentage: number;
          /** @enum {string} */
          program: "phantom";
          /**
           * @description The user wallet address which requested the quote from the wallet.
           * @example DUcTi3rDyS5QEmZ4BNRBejtArmDCWaPYGfN44vBJXKL5
           */
          user_wallet_address: string;
        }
    ) & {
      buy_tokens: components["schemas"]["TokenAmountSvm"][];
      /**
       * @description The chain id where the opportunity will be executed.
       * @example solana
       */
      chain_id: string;
      /**
       * @description The permission account to be permitted by the ER contract for the opportunity execution of the protocol.
       * @example DUcTi3rDyS5QEmZ4BNRBejtArmDCWaPYGfN44vBJXKL5
       */
      permission_account: string;
      /**
       * @description The router account to be used for the opportunity execution of the protocol.
       * @example DUcTi3rDyS5QEmZ4BNRBejtArmDCWaPYGfN44vBJXKL5
       */
      router: string;
      sell_tokens: components["schemas"]["TokenAmountSvm"][];
      /**
       * Format: int64
       * @description The slot where the program params were fetched from using the RPC.
       * @example 293106477
       */
      slot: number;
    };
    /** @description The input type for deleting opportunities. */
    OpportunityDelete:
      | (components["schemas"]["OpportunityDeleteSvm"] & {
          /** @enum {string} */
          chain_type: "svm";
        })
      | (components["schemas"]["OpportunityDeleteEvm"] & {
          /** @enum {string} */
          chain_type: "evm";
        });
    OpportunityDeleteEvm: components["schemas"]["OpportunityDeleteV1Evm"] & {
      /** @enum {string} */
      version: "v1";
    };
    OpportunityDeleteSvm: components["schemas"]["OpportunityDeleteV1Svm"] & {
      /** @enum {string} */
      version: "v1";
    };
    /** @description Opportunity parameters needed for deleting live opportunities. */
    OpportunityDeleteV1Evm: {
      /**
       * @description The chain id for the opportunity.
       * @example solana
       */
      chain_id: string;
      /**
       * @description The permission key of the opportunity.
       * @example 0xdeadbeefcafe
       */
      permission_key: string;
    };
    /** @description Opportunity parameters needed for deleting live opportunities. */
    OpportunityDeleteV1Svm: {
      /**
       * @description The chain id for the opportunity.
       * @example solana
       */
      chain_id: string;
      /**
       * @description The permission account for the opportunity.
       * @example DUcTi3rDyS5QEmZ4BNRBejtArmDCWaPYGfN44vBJXKL5
       */
      permission_account: string;
      program: components["schemas"]["ProgramSvm"];
      /**
       * @description The router account for the opportunity.
       * @example DUcTi3rDyS5QEmZ4BNRBejtArmDCWaPYGfN44vBJXKL5
       */
      router: string;
    };
    OpportunityEvm: (components["schemas"]["OpportunityParamsV1Evm"] & {
      /** @enum {string} */
      version: "v1";
    }) & {
      /**
       * @description Creation time of the opportunity (in microseconds since the Unix epoch).
       * @example 1700000000000000
       */
      creation_time: number;
      /**
       * @description The opportunity unique id.
       * @example obo3ee3e-58cc-4372-a567-0e02b2c3d479
       */
      opportunity_id: string;
    };
    /** @enum {string} */
    OpportunityMode: "live" | "historical";
    OpportunityParamsEvm: components["schemas"]["OpportunityParamsV1Evm"] & {
      /** @enum {string} */
      version: "v1";
    };
    OpportunityParamsSvm: components["schemas"]["OpportunityParamsV1Svm"] & {
      /** @enum {string} */
      version: "v1";
    };
    OpportunityParamsV1Evm: components["schemas"]["OpportunityCreateV1Evm"];
    /**
     * @description Opportunity parameters needed for on-chain execution.
     * Parameters may differ for each program.
     */
    OpportunityParamsV1Svm: (
      | {
          /**
           * @description The Limo order to be executed, encoded in base64.
           * @example DUcTi3rDyS5QEmZ4BNRBejtArmDCWaPYGfN44vBJXKL5
           */
          order: string;
          /**
           * @description Address of the order account.
           * @example DUcTi3rDyS5QEmZ4BNRBejtArmDCWaPYGfN44vBJXKL5
           */
          order_address: string;
          /** @enum {string} */
          program: "limo";
        }
      | {
          buy_token: components["schemas"]["TokenAmountSvm"];
          /**
           * Format: double
           * @description The maximum slippage percentage that the user is willing to accept.
           * @example 0.5
           */
          maximum_slippage_percentage: number;
          /**
           * @description The permission account to be permitted by the ER contract for the opportunity execution of the protocol.
           * @example DUcTi3rDyS5QEmZ4BNRBejtArmDCWaPYGfN44vBJXKL5
           */
          permission_account: string;
          /** @enum {string} */
          program: "phantom";
          /**
           * @description The router account to be used for the opportunity execution of the protocol.
           * @example DUcTi3rDyS5QEmZ4BNRBejtArmDCWaPYGfN44vBJXKL5
           */
          router_account: string;
          sell_token: components["schemas"]["TokenAmountSvm"];
          /**
           * @description The user wallet address which requested the quote from the wallet.
           * @example DUcTi3rDyS5QEmZ4BNRBejtArmDCWaPYGfN44vBJXKL5
           */
          user_wallet_address: string;
        }
    ) & {
      /** @example solana */
      chain_id: string;
    };
    OpportunitySvm: (components["schemas"]["OpportunityParamsV1Svm"] & {
      /** @enum {string} */
      version: "v1";
    }) & {
      /**
       * @description Creation time of the opportunity (in microseconds since the Unix epoch).
       * @example 1700000000000000
       */
      creation_time: number;
      /**
       * @description The opportunity unique id.
       * @example obo3ee3e-58cc-4372-a567-0e02b2c3d479
       */
      opportunity_id: string;
      /**
       * Format: int64
       * @description The slot where the program params were fetched from using the RPC.
       * @example 293106477
       */
      slot: number;
    };
    /** @enum {string} */
    ProgramSvm: "phantom" | "limo";
    Quote: components["schemas"]["QuoteSvm"];
    QuoteCreate: components["schemas"]["QuoteCreateSvm"];
    /**
     * @description Parameters needed to create a new opportunity from the Phantom wallet.
     * Auction server will extract the output token price for the auction.
     */
    QuoteCreatePhantomV1Svm: {
      /**
       * @description The chain id for creating the quote.
       * @example solana
       */
      chain_id: string;
      /**
       * Format: int64
       * @description The input token amount that the user wants to swap.
       * @example 100
       */
      input_token_amount: number;
      /**
       * @description The token mint address of the input token.
       * @example EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v
       */
      input_token_mint: string;
      /**
       * Format: double
       * @description The maximum slippage percentage that the user is willing to accept.
       * @example 0.5
       */
      maximum_slippage_percentage: number;
      /**
       * @description The token mint address of the output token.
       * @example EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v
       */
      output_token_mint: string;
      /**
       * @description The user wallet address which requested the quote from the wallet.
       * @example DUcTi3rDyS5QEmZ4BNRBejtArmDCWaPYGfN44vBJXKL5
       */
      user_wallet_address: string;
    };
    QuoteCreateSvm: components["schemas"]["QuoteCreateV1Svm"] & {
      /** @enum {string} */
      version: "v1";
    };
    QuoteCreateV1Svm: components["schemas"]["QuoteCreatePhantomV1Svm"] & {
      /** @enum {string} */
      program: "phantom";
    };
    QuoteSvm: components["schemas"]["QuoteV1Svm"] & {
      /** @enum {string} */
      version: "v1";
    };
    QuoteV1Svm: {
      /**
       * @description The chain id for the quote.
       * @example solana
       */
      chain_id: string;
      /**
       * Format: int64
       * @description The expiration time of the quote (in seconds since the Unix epoch).
       * @example 1700000000000000
       */
      expiration_time: number;
      input_token: components["schemas"]["TokenAmountSvm"];
      /**
       * Format: double
       * @description The maximum slippage percentage that the user is willing to accept.
       * @example 0.5
       */
      maximum_slippage_percentage: number;
      output_token: components["schemas"]["TokenAmountSvm"];
      /**
       * @description The signed transaction for the quote to be executed on chain which is valid until the expiration time.
       * @example SGVsbG8sIFdvcmxkIQ==
       */
      transaction: string;
    };
    ServerResultMessage:
      | {
          result: components["schemas"]["APIResponse"] | null;
          /** @enum {string} */
          status: "success";
        }
      | {
          result: string;
          /** @enum {string} */
          status: "error";
        };
    /**
     * @description This enum is used to send the result for a specific client request with the same id.
     * Id is only None when the client message is invalid.
     */
    ServerResultResponse: components["schemas"]["ServerResultMessage"] & {
      id?: string | null;
    };
    /** @description This enum is used to send an update to the client for any subscriptions made. */
    ServerUpdateResponse:
      | {
          opportunity: components["schemas"]["Opportunity"];
          /** @enum {string} */
          type: "new_opportunity";
        }
      | {
          status: components["schemas"]["BidStatusWithId"];
          /** @enum {string} */
          type: "bid_status_update";
        }
      | {
          /** @enum {string} */
          type: "svm_chain_update";
          update: components["schemas"]["SvmChainUpdate"];
        }
      | {
          opportunity_delete: components["schemas"]["OpportunityDelete"];
          /** @enum {string} */
          type: "remove_opportunities";
        };
    SimulatedBid:
      | components["schemas"]["SimulatedBidEvm"]
      | components["schemas"]["SimulatedBidSvm"];
    /** BidResponseEvm */
    SimulatedBidEvm: {
      /**
       * @description The chain id for bid.
       * @example op_sepolia
       */
      chain_id: string;
      /**
       * @description The unique id for bid.
       * @example obo3ee3e-58cc-4372-a567-0e02b2c3d479
       */
      id: string;
      /**
       * @description The time server received the bid formatted in rfc3339.
       * @example 2024-05-23T21:26:57.329954Z
       */
      initiation_time: string;
      /**
       * @description The profile id for the bid owner.
       * @example obo3ee3e-58cc-4372-a567-0e02b2c3d479
       */
      profile_id: string;
    } & {
      /**
       * @description Amount of bid in wei.
       * @example 10
       */
      bid_amount: string;
      /**
       * @description The gas limit for the contract call.
       * @example 2000000
       */
      gas_limit: string;
      /**
       * @description The permission key for bid.
       * @example 0xdeadbeef
       */
      permission_key: string;
      status: components["schemas"]["BidStatusEvm"];
      /**
       * @description Calldata for the contract call.
       * @example 0xdeadbeef
       */
      target_calldata: string;
      /**
       * @description The contract address to call.
       * @example 0xcA11bde05977b3631167028862bE2a173976CA11
       */
      target_contract: string;
    };
    /** BidResponseSvm */
    SimulatedBidSvm: {
      /**
       * @description The chain id for bid.
       * @example op_sepolia
       */
      chain_id: string;
      /**
       * @description The unique id for bid.
       * @example obo3ee3e-58cc-4372-a567-0e02b2c3d479
       */
      id: string;
      /**
       * @description The time server received the bid formatted in rfc3339.
       * @example 2024-05-23T21:26:57.329954Z
       */
      initiation_time: string;
      /**
       * @description The profile id for the bid owner.
       * @example obo3ee3e-58cc-4372-a567-0e02b2c3d479
       */
      profile_id: string;
    } & {
      /**
       * Format: int64
       * @description Amount of bid in lamports.
       * @example 1000
       */
      bid_amount: number;
      /**
       * @description The permission key for bid in base64 format.
       * This is the concatenation of the permission account and the router account.
       * @example DUcTi3rDyS5QEmZ4BNRBejtArmDCWaPYGfN44vBJXKL5
       */
      permission_key: string;
      status: components["schemas"]["BidStatusSvm"];
      /**
       * @description The transaction of the bid.
       * @example SGVsbG8sIFdvcmxkIQ==
       */
      transaction: string;
    };
    /** BidsResponse */
    SimulatedBids: {
      items: components["schemas"]["SimulatedBid"][];
    };
    SvmChainUpdate: {
      blockhash: components["schemas"]["Hash"];
      chain_id: components["schemas"]["ChainId"];
    };
    TokenAmountEvm: {
      /**
       * @description The token amount.
       * @example 1000
       */
      amount: string;
      /**
       * @description The token contract address.
       * @example 0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2
       */
      token: string;
    };
    TokenAmountSvm: {
      /**
       * Format: int64
       * @description The token amount in lamports.
       * @example 1000
       */
      amount: number;
      /**
       * @description The token contract address.
       * @example DUcTi3rDyS5QEmZ4BNRBejtArmDCWaPYGfN44vBJXKL5
       */
      token: string;
    };
  };
  responses: {
    BidResult: {
      content: {
        "application/json": {
          /**
           * @description The unique id created to identify the bid. This id can be used to query the status of the bid.
           * @example beedbeed-58cc-4372-a567-0e02b2c3d479
           */
          id: string;
          /**
           * @description The status of the request. If the bid was placed successfully, the status will be "OK".
           * @example OK
           */
          status: string;
        };
      };
    };
    /** @description An error occurred processing the request */
    ErrorBodyResponse: {
      content: {
        "application/json": {
          error: string;
        };
      };
    };
    Opportunity: {
      content: {
        "application/json":
          | components["schemas"]["OpportunityEvm"]
          | components["schemas"]["OpportunitySvm"];
      };
    };
    SimulatedBids: {
      content: {
        "application/json": {
          items: components["schemas"]["SimulatedBid"][];
        };
      };
    };
  };
  parameters: never;
  requestBodies: never;
  headers: never;
  pathItems: never;
}

export type $defs = Record<string, never>;

export type external = Record<string, never>;

export interface operations {
  /**
   * Returns at most 20 bids which were submitted after a specific time.
   * @description If no time is provided, the server will return the first bids.
   */
  get_bids_by_time: {
    parameters: {
      query?: {
        /** @example 2024-05-23T21:26:57.329954Z */
        from_time?: string | null;
      };
    };
    responses: {
      /** @description Paginated list of bids for the specified query */
      200: {
        content: {
          "application/json": components["schemas"]["SimulatedBids"];
        };
      };
      400: components["responses"]["ErrorBodyResponse"];
    };
  };
  /**
   * Bid on a specific permission key for a specific chain.
   * @description Your bid will be verified by the server. Depending on the outcome of the auction, a transaction
   * containing your bid will be sent to the blockchain expecting the bid amount to be paid in the transaction.
   */
  bid: {
    requestBody: {
      content: {
        "application/json": components["schemas"]["Bid"];
      };
    };
    responses: {
      /** @description Bid was placed successfully */
      200: {
        content: {
          "application/json": components["schemas"]["BidResult"];
        };
      };
      400: components["responses"]["ErrorBodyResponse"];
      /** @description Chain id was not found */
      404: {
        content: {
          "application/json": components["schemas"]["ErrorBodyResponse"];
        };
      };
    };
  };
  /** Query the status of a specific bid. */
  bid_status: {
    parameters: {
      path: {
        /** @description Bid id to query for */
        bid_id: string;
      };
    };
    responses: {
      200: {
        content: {
          "application/json": components["schemas"]["BidStatus"];
        };
      };
      400: components["responses"]["ErrorBodyResponse"];
      /** @description Bid was not found */
      404: {
        content: {
          "application/json": components["schemas"]["ErrorBodyResponse"];
        };
      };
    };
  };
  /**
   * Fetch opportunities ready for execution or historical opportunities
   * @description depending on the mode. You need to provide `chain_id` for historical mode.
   * Opportunities are sorted by creation time in ascending order.
   * Total number of opportunities returned is limited by 20.
   */
  get_opportunities: {
    parameters: {
      query?: {
        /** @example op_sepolia */
        chain_id?: string | null;
        /** @description Get opportunities in live or historical mode. */
        mode?: components["schemas"]["OpportunityMode"];
        /**
         * @description The permission key to filter the opportunities by. Used only in historical mode.
         * @example 0xdeadbeef
         */
        permission_key?: string | null;
        /**
         * @description The time to get the opportunities from.
         * @example 2024-05-23T21:26:57.329954Z
         */
        from_time?: string | null;
        /**
         * @description The maximum number of opportunities to return. Capped at 100.
         * @example 20
         */
        limit?: number;
      };
    };
    responses: {
      /** @description Array of opportunities ready for bidding */
      200: {
        content: {
          "application/json": components["schemas"]["Opportunity"][];
        };
      };
      400: components["responses"]["ErrorBodyResponse"];
      /** @description Chain id was not found */
      404: {
        content: {
          "application/json": components["schemas"]["ErrorBodyResponse"];
        };
      };
    };
  };
  /**
   * Submit an opportunity ready to be executed.
   * @description The opportunity will be verified by the server. If the opportunity is valid, it will be stored in the database
   * and will be available for bidding.
   */
  post_opportunity: {
    requestBody: {
      content: {
        "application/json": components["schemas"]["OpportunityCreate"];
      };
    };
    responses: {
      /** @description The created opportunity */
      200: {
        content: {
          "application/json": components["schemas"]["Opportunity"];
        };
      };
      400: components["responses"]["ErrorBodyResponse"];
      /** @description Chain id was not found */
      404: {
        content: {
          "application/json": components["schemas"]["ErrorBodyResponse"];
        };
      };
    };
  };
  /** Delete all opportunities for specified data. */
  delete_opportunities: {
    requestBody: {
      content: {
        "application/json": components["schemas"]["OpportunityDelete"];
      };
    };
    responses: {
      /** @description Opportunities deleted successfully */
      204: {
        content: never;
      };
      400: components["responses"]["ErrorBodyResponse"];
      /** @description Chain id was not found */
      404: {
        content: {
          "application/json": components["schemas"]["ErrorBodyResponse"];
        };
      };
    };
  };
  /**
   * Submit a quote request.
   * @description The server will estimate the quote price, which will be used to create an opportunity.
   * After a certain time, searcher bids are collected, the winning signed bid will be returned along with the estimated price.
   */
  post_quote: {
    requestBody: {
      content: {
        "application/json": components["schemas"]["QuoteCreate"];
      };
    };
    responses: {
      /** @description The created quote */
      200: {
        content: {
          "application/json": components["schemas"]["Quote"];
        };
      };
      400: components["responses"]["ErrorBodyResponse"];
      /** @description No quote available right now */
      404: {
        content: {
          "application/json": components["schemas"]["ErrorBodyResponse"];
        };
      };
    };
  };
  /** Bid on opportunity. */
  opportunity_bid: {
    parameters: {
      path: {
        /** @description Opportunity id to bid on */
        opportunity_id: string;
      };
    };
    requestBody: {
      content: {
        "application/json": components["schemas"]["OpportunityBidEvm"];
      };
    };
    responses: {
      /** @description Bid Result */
      200: {
        content: {
          "application/json": components["schemas"]["OpportunityBidResult"];
        };
      };
      400: components["responses"]["ErrorBodyResponse"];
      /** @description Opportunity or chain id was not found */
      404: {
        content: {
          "application/json": components["schemas"]["ErrorBodyResponse"];
        };
      };
    };
  };
  /**
   * Revoke the authenticated profile access token.
   * @description Returns empty response.
   */
  delete_profile_access_token: {
    responses: {
      /** @description The token successfully revoked */
      200: {
        content: never;
      };
      400: components["responses"]["ErrorBodyResponse"];
    };
  };
}
