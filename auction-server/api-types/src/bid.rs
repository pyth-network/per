use {
    crate::{
        opportunity::OpportunityId,
        profile::ProfileId,
        AccessLevel,
        ChainId,
        PermissionKeySvm,
        Routable,
    },
    serde::{
        Deserialize,
        Serialize,
    },
    serde_with::{
        serde_as,
        DisplayFromStr,
    },
    solana_sdk::{
        clock::Slot,
        signature::Signature,
        transaction::VersionedTransaction,
    },
    strum::AsRefStr,
    time::OffsetDateTime,
    utoipa::{
        IntoParams,
        ToResponse,
        ToSchema,
    },
    uuid::Uuid,
};

pub type BidId = Uuid;
pub type BidAmountSvm = u64;

#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum SubmissionFailedReason {
    /// The bid was cancelled by the owner.
    Cancelled,
    /// The bid was submitted very late.
    DeadlinePassed,
}

#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum BidFailedReason {
    /// The user didn't have enough funds when the transaction landed
    InsufficientUserFunds,
    /// The searcher didn't have enough funds when the transaction landed
    InsufficientSearcherFunds,
    /// A SOL transfer failed with insufficient funds, for example because the user didn't have enough SOL to wrap
    InsufficientFundsSolTransfer,
    /// The transaction landed after the deadline
    DeadlinePassed,
    /// Other reasons
    Other,
}

#[serde_as]
#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BidStatusSvm {
    /// The temporary state which means the auction for this bid is pending.
    /// It will be updated to AwaitingSignature, Lost or Submitted after the auction takes place.
    #[schema(title = "Pending")]
    Pending,
    /// The bid is waiting for the remaining parties to sign the transaction.
    /// After all parties sign, the bid will be submitted to the chain.
    /// During this state, the bid can be canceled by the owner.
    #[schema(title = "AwaitingSignature")]
    AwaitingSignature {
        #[schema(example = "Jb2urXPyEh4xiBgzYvwEFe4q1iMxG1DNxWGGQg94AmKgqFTwLAiTiHrYiYxwHUB4DV8u5ahNEVtMMDm3sNSRdTg", value_type = String)]
        #[serde_as(as = "DisplayFromStr")]
        result: Signature,
    },
    /// The bid lost the auction.
    /// This bid status will have a result field containing the signature of the transaction corresponding to the winning bid,
    /// unless the auction had no winner (because all bids were found to be invalid).
    #[schema(title = "Lost")]
    Lost {
        #[schema(example = "Jb2urXPyEh4xiBgzYvwEFe4q1iMxG1DNxWGGQg94AmKgqFTwLAiTiHrYiYxwHUB4DV8u5ahNEVtMMDm3sNSRdTg", value_type = Option<String>)]
        #[serde(with = "crate::serde::nullable_signature_svm")]
        result: Option<Signature>,
    },
    /// The bid was sent to the user for submission. In this case, the user has the last look and may decide to cancel by not signing and broadcasting the transaction.
    #[schema(title = "SentToUserForSubmission")]
    SentToUserForSubmission {
        #[schema(example = "Jb2urXPyEh4xiBgzYvwEFe4q1iMxG1DNxWGGQg94AmKgqFTwLAiTiHrYiYxwHUB4DV8u5ahNEVtMMDm3sNSRdTg", value_type = String)]
        #[serde_as(as = "DisplayFromStr")]
        result: Signature,
    },
    /// The bid won the auction and was submitted to the chain, with the signature of the corresponding transaction provided in the result field.
    /// This state is temporary and will be updated to either Won or Failed after the transaction is included in a block, or Expired if the transaction expires before it is included.
    #[schema(title = "Submitted")]
    Submitted {
        #[schema(example = "Jb2urXPyEh4xiBgzYvwEFe4q1iMxG1DNxWGGQg94AmKgqFTwLAiTiHrYiYxwHUB4DV8u5ahNEVtMMDm3sNSRdTg", value_type = String)]
        #[serde_as(as = "DisplayFromStr")]
        result: Signature,
    },
    /// The bid won the auction and was included in a block successfully.
    #[schema(title = "Won")]
    Won {
        #[schema(example = "Jb2urXPyEh4xiBgzYvwEFe4q1iMxG1DNxWGGQg94AmKgqFTwLAiTiHrYiYxwHUB4DV8u5ahNEVtMMDm3sNSRdTg", value_type = String)]
        #[serde_as(as = "DisplayFromStr")]
        result: Signature,
    },
    /// The bid was submitted on-chain, was included in a block, but resulted in a failed transaction.
    #[schema(title = "Failed")]
    Failed {
        #[schema(example = "Jb2urXPyEh4xiBgzYvwEFe4q1iMxG1DNxWGGQg94AmKgqFTwLAiTiHrYiYxwHUB4DV8u5ahNEVtMMDm3sNSRdTg", value_type = String)]
        #[serde_as(as = "DisplayFromStr")]
        result: Signature,
        reason: Option<BidFailedReason>,
    },
    /// The bid was submitted on-chain but expired before it was included in a block.
    #[schema(title = "Expired")]
    Expired {
        #[schema(example = "Jb2urXPyEh4xiBgzYvwEFe4q1iMxG1DNxWGGQg94AmKgqFTwLAiTiHrYiYxwHUB4DV8u5ahNEVtMMDm3sNSRdTg", value_type = String)]
        #[serde_as(as = "DisplayFromStr")]
        result: Signature,
    },
    /// The bid was canceled by the owner.
    /// Owner can only cancel the bid when the bid is in AwaitingSignature state.
    #[schema(title = "Cancelled")]
    Cancelled {
        #[schema(example = "Jb2urXPyEh4xiBgzYvwEFe4q1iMxG1DNxWGGQg94AmKgqFTwLAiTiHrYiYxwHUB4DV8u5ahNEVtMMDm3sNSRdTg", value_type = String)]
        #[serde_as(as = "DisplayFromStr")]
        result: Signature,
    },
    /// The Submission of the bid failed.
    /// This can happen if the user try to submit the bid after it being cancelled by the owner.
    /// Or if the user try to submit the bid late, while the bid is in the state of AwaitingSignature.
    #[schema(title = "submission_failed")]
    SubmissionFailed {
        #[schema(example = "Jb2urXPyEh4xiBgzYvwEFe4q1iMxG1DNxWGGQg94AmKgqFTwLAiTiHrYiYxwHUB4DV8u5ahNEVtMMDm3sNSRdTg", value_type = String)]
        #[serde_as(as = "DisplayFromStr")]
        result: Signature,
        reason: SubmissionFailedReason,
    },
}

#[derive(Serialize, Deserialize, ToSchema, Clone, PartialEq, Debug)]
#[serde(untagged)]
pub enum BidStatus {
    Svm(BidStatusSvm),
}

#[derive(Serialize, Deserialize, ToResponse, ToSchema, Clone, Debug)]
pub struct BidResult {
    /// The status of the request. If the bid was placed successfully, the status will be "OK".
    #[schema(example = "OK")]
    pub status: String,
    /// The unique id created to identify the bid. This id can be used to query the status of the bid.
    #[schema(example = "beedbeed-58cc-4372-a567-0e02b2c3d479", value_type=String)]
    pub id:     BidId,
}

#[derive(Clone, Debug, ToSchema, Serialize, Deserialize)]
pub struct BidCoreFields {
    /// The unique id for bid.
    #[schema(example = "obo3ee3e-58cc-4372-a567-0e02b2c3d479", value_type = String)]
    pub id:              BidId,
    /// The chain id for bid.
    #[schema(example = "op_sepolia", value_type = String)]
    pub chain_id:        ChainId,
    /// The time server received the bid formatted in rfc3339.
    #[schema(example = "2024-05-23T21:26:57.329954Z", value_type = String)]
    #[serde(with = "time::serde::rfc3339")]
    pub initiation_time: OffsetDateTime,
    /// The profile id for the bid owner.
    #[schema(example = "obo3ee3e-58cc-4372-a567-0e02b2c3d479", value_type = String)]
    pub profile_id:      Option<ProfileId>,
}

#[derive(Clone, Debug, ToSchema, Serialize, Deserialize)]
pub struct BidSvm {
    #[serde(flatten)]
    #[schema(inline)]
    pub core_fields:    BidCoreFields,
    /// The latest status for bid.
    pub status:         BidStatusSvm,
    /// The transaction of the bid.
    #[schema(example = "SGVsbG8sIFdvcmxkIQ==", value_type = String)]
    #[serde(with = "crate::serde::transaction_svm")]
    pub transaction:    VersionedTransaction,
    /// Amount of bid in lamports.
    #[schema(example = "1000", value_type = u64)]
    pub bid_amount:     BidAmountSvm,
    /// The permission key for bid in base64 format.
    /// This is the concatenation of the opportunity type, the router, and the permission account.
    #[schema(example = "DUcTi3rDyS5QEmZ4BNRBejtArmDCWaPYGfN44vBJXKL5", value_type = String)]
    pub permission_key: PermissionKeySvm,
}

#[derive(Clone, Debug, ToSchema, Serialize, Deserialize)]
#[serde(untagged)]
#[allow(clippy::large_enum_variant)]
pub enum Bid {
    Svm(BidSvm),
}

impl Bid {
    pub fn get_initiation_time(&self) -> OffsetDateTime {
        match self {
            Bid::Svm(bid) => bid.core_fields.initiation_time,
        }
    }

    pub fn get_status(&self) -> BidStatus {
        match self {
            Bid::Svm(bid) => BidStatus::Svm(bid.status.clone()),
        }
    }
}

#[derive(Serialize, Deserialize, ToSchema, Clone, Debug)]
pub struct BidCreateOnChainSvm {
    /// The chain id to bid on.
    #[schema(example = "solana", value_type = String)]
    pub chain_id:    ChainId,
    /// The transaction for bid.
    #[schema(example = "SGVsbG8sIFdvcmxkIQ==", value_type = String)]
    #[serde(with = "crate::serde::transaction_svm")]
    pub transaction: VersionedTransaction,
    /// The minimum slot required for the bid to be executed successfully
    /// None if the bid can be executed at any recent slot
    #[schema(example = 293106477, value_type = Option<u64>)]
    pub slot:        Option<Slot>,
}

#[derive(Serialize, Deserialize, ToSchema, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub enum BidCreateSwapSvmTag {
    Swap,
}

#[derive(Serialize, Deserialize, ToSchema, Clone, Debug)]
pub struct BidCreateSwapSvm {
    /// The chain id to bid on.
    #[schema(example = "solana", value_type = String)]
    pub chain_id:       ChainId,
    /// The transaction for bid.
    #[schema(example = "SGVsbG8sIFdvcmxkIQ==", value_type = String)]
    #[serde(with = "crate::serde::transaction_svm")]
    pub transaction:    VersionedTransaction,
    /// The id of the swap opportunity to bid on.
    #[schema(example = "obo3ee3e-58cc-4372-a567-0e02b2c3d479", value_type = String)]
    pub opportunity_id: OpportunityId,
    /// The bid type. Should be "swap"
    #[schema(example = "swap")]
    #[serde(rename = "type")]
    pub _type:          BidCreateSwapSvmTag, // this is mainly to distinguish next types of bids in the future
}

#[derive(Serialize, Deserialize, ToSchema, Debug, Clone)]
#[serde(untagged)]
pub enum BidCreateSvm {
    Swap(BidCreateSwapSvm),
    OnChain(BidCreateOnChainSvm),
}

#[derive(Serialize, Deserialize, ToSchema, Debug, Clone)]
#[serde(untagged)] // Remove tags to avoid key-value wrapping
pub enum BidCreate {
    Svm(BidCreateSvm),
}

#[derive(Serialize, Clone, ToSchema, ToResponse, Deserialize, Debug, PartialEq)]
pub struct BidStatusWithId {
    #[schema(value_type = String)]
    pub id:         BidId,
    pub bid_status: BidStatus,
}

#[derive(Serialize, Deserialize, IntoParams, Clone)]
pub struct GetBidStatusParams {
    #[param(example="op_sepolia", value_type = String)]
    pub chain_id: ChainId,

    #[param(example="obo3ee3e-58cc-4372-a567-0e02b2c3d479", value_type = String)]
    pub bid_id: BidId,
}

#[derive(Serialize, Deserialize, ToResponse, ToSchema, Clone)]
pub struct Bids {
    pub items: Vec<Bid>,
}

#[derive(Serialize, Deserialize, IntoParams)]
pub struct GetBidsByTimeQueryParams {
    #[param(example="2024-05-23T21:26:57.329954Z", value_type = Option<String>)]
    #[serde(default, with = "crate::serde::nullable_datetime")]
    pub from_time: Option<OffsetDateTime>,
}

impl BidCreate {
    pub fn get_chain_id(&self) -> ChainId {
        match self {
            BidCreate::Svm(BidCreateSvm::Swap(bid_create_svm)) => bid_create_svm.chain_id.clone(),
            BidCreate::Svm(BidCreateSvm::OnChain(bid_create_svm)) => {
                bid_create_svm.chain_id.clone()
            }
        }
    }
}

// We get clippy warning when we use AsRefStr macro with deprecated.
// Disabled the strum for deprecated routes to avoid clippy warnings.
#[derive(AsRefStr, Clone)]
#[strum(prefix = "/")]
pub enum Route {
    #[strum(serialize = "")]
    PostBid,
    #[strum(serialize = "")]
    GetBidsByTime,
    #[strum(serialize = ":bid_id")]
    GetBidStatus,
    #[strum(serialize = "submit")]
    PostSubmitQuote,
    #[strum(serialize = ":bid_id/cancel")]
    PostCancelBid,
}

#[derive(Clone)]
#[deprecated = "Use Route instead"]
pub enum DeprecatedRoute {
    DeprecatedGetBidsByTime,
    DeprecatedGetBidStatus,
}

impl Routable for Route {
    fn properties(&self) -> crate::RouteProperties {
        let prefix = match self {
            Route::PostSubmitQuote => crate::Route::Quote.as_ref(),
            _ => crate::Route::Bid.as_ref(),
        };
        let full_path = format!("{}{}{}", crate::Route::V1.as_ref(), prefix, self.as_ref())
            .trim_end_matches('/')
            .to_string();

        let full_path_with_chain = format!(
            "{}{}{}",
            crate::Route::V1Chain.as_ref(),
            prefix,
            self.as_ref()
        )
        .trim_end_matches('/')
        .to_string();

        match self {
            Route::PostBid => crate::RouteProperties {
                method: http::Method::POST,
                access_level: AccessLevel::Public,
                full_path,
            },
            Route::GetBidsByTime => crate::RouteProperties {
                method:       http::Method::GET,
                access_level: AccessLevel::LoggedIn,
                full_path:    full_path_with_chain,
            },
            Route::GetBidStatus => crate::RouteProperties {
                method:       http::Method::GET,
                access_level: AccessLevel::Public,
                full_path:    full_path_with_chain,
            },
            Route::PostSubmitQuote => crate::RouteProperties {
                method:       http::Method::POST,
                access_level: AccessLevel::Public,
                full_path:    full_path_with_chain,
            },
            Route::PostCancelBid => crate::RouteProperties {
                method:       http::Method::POST,
                access_level: AccessLevel::LoggedIn,
                full_path:    full_path_with_chain,
            },
        }
    }
}

#[allow(deprecated)]
impl AsRef<str> for DeprecatedRoute {
    fn as_ref(&self) -> &str {
        match self {
            DeprecatedRoute::DeprecatedGetBidStatus => "/:bid_id",
            DeprecatedRoute::DeprecatedGetBidsByTime => "/",
        }
    }
}

#[allow(deprecated)]
impl Routable for DeprecatedRoute {
    fn properties(&self) -> crate::RouteProperties {
        let full_path = format!(
            "{}{}{}",
            crate::Route::V1.as_ref(),
            crate::Route::Bid.as_ref(),
            self.as_ref(),
        )
        .trim_end_matches('/')
        .to_string();

        match self {
            DeprecatedRoute::DeprecatedGetBidsByTime => crate::RouteProperties {
                method: http::Method::GET,
                access_level: AccessLevel::LoggedIn,
                full_path,
            },
            DeprecatedRoute::DeprecatedGetBidStatus => crate::RouteProperties {
                method: http::Method::GET,
                access_level: AccessLevel::Public,
                full_path,
            },
        }
    }
}

#[derive(Serialize, Deserialize, IntoParams, Clone)]
pub struct BidCancelParams {
    /// The chain id of the bid to cancel.
    #[param(example="solana", value_type = String)]
    pub chain_id: ChainId,
    /// The id of the bid to cancel.
    #[param(example="obo3ee3e-58cc-4372-a567-0e02b2c3d479", value_type = String)]
    pub bid_id:   BidId,
}

#[derive(Serialize, Deserialize, ToSchema, Debug, Clone)]
pub struct BidCancelSvm {
    /// The chain id of the bid to cancel.
    #[schema(example = "solana", value_type = String)]
    pub chain_id: ChainId,
    /// The id of the bid to cancel.
    #[schema(example="obo3ee3e-58cc-4372-a567-0e02b2c3d479", value_type = String)]
    pub bid_id:   BidId,
}

#[derive(Serialize, Deserialize, ToSchema, Debug, Clone)]
#[serde(untagged)] // Remove tags to avoid key-value wrapping
pub enum BidCancel {
    Svm(BidCancelSvm),
}
