/*
 * auction-server
 *
 * No description provided (generated by Openapi Generator https://github.com/openapitools/openapi-generator)
 *
 * The version of the OpenAPI document: 0.14.0
 *
 * Generated by: https://openapi-generator.tech
 */

use {
    crate::models,
    serde::{
        Deserialize,
        Serialize,
    },
};

/// QuoteCreatePhantomV1Svm : Parameters needed to create a new opportunity from the Phantom wallet. Auction server will extract the output token price for the auction.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct QuoteCreatePhantomV1Svm {
    /// The chain id for creating the quote.
    #[serde(rename = "chain_id")]
    pub chain_id:                    String,
    /// The input token amount that the user wants to swap.
    #[serde(rename = "input_token_amount")]
    pub input_token_amount:          i64,
    /// The token mint address of the input token.
    #[serde(rename = "input_token_mint")]
    pub input_token_mint:            String,
    /// The maximum slippage percentage that the user is willing to accept.
    #[serde(rename = "maximum_slippage_percentage")]
    pub maximum_slippage_percentage: f64,
    /// The token mint address of the output token.
    #[serde(rename = "output_token_mint")]
    pub output_token_mint:           String,
    /// The user wallet address which requested the quote from the wallet.
    #[serde(rename = "user_wallet_address")]
    pub user_wallet_address:         String,
}

impl QuoteCreatePhantomV1Svm {
    /// Parameters needed to create a new opportunity from the Phantom wallet. Auction server will extract the output token price for the auction.
    pub fn new(
        chain_id: String,
        input_token_amount: i64,
        input_token_mint: String,
        maximum_slippage_percentage: f64,
        output_token_mint: String,
        user_wallet_address: String,
    ) -> QuoteCreatePhantomV1Svm {
        QuoteCreatePhantomV1Svm {
            chain_id,
            input_token_amount,
            input_token_mint,
            maximum_slippage_percentage,
            output_token_mint,
            user_wallet_address,
        }
    }
}
