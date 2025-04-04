use {
    serde::{
        Deserialize,
        Serialize,
    },
    serde_with::{
        serde_as,
        DisplayFromStr,
    },
    solana_sdk::{
        signature::Signature,
        transaction::VersionedTransaction,
    },
    utoipa::ToSchema,
    uuid::Uuid,
};

/// Parameters needed to submit a quote from server.
#[serde_as]
#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug)]
pub struct SubmitQuote {
    /// The reference id for the quote that should be submitted.
    #[schema(example = "beedbeed-58cc-4372-a567-0e02b2c3d479", value_type=String)]
    pub reference_id:   Uuid,
    /// The signature of the user for the quote.
    #[schema(example = "Jb2urXPyEh4xiBgzYvwEFe4q1iMxG1DNxWGGQg94AmKgqFTwLAiTiHrYiYxwHUB4DV8u5ahNEVtMMDm3sNSRdTg", value_type = String)]
    #[serde_as(as = "DisplayFromStr")]
    pub user_signature: Signature,
}

/// Response to a quote submission.
#[serde_as]
#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug)]
pub struct SubmitQuoteResponse {
    /// The fully signed versioned transaction that was submitted.
    /// The transaction is encoded in base64.
    #[schema(example = "SGVsbG8sIFdvcmxkIQ==", value_type = String)]
    #[serde(with = "crate::serde::transaction_svm")]
    pub transaction: VersionedTransaction,
}
