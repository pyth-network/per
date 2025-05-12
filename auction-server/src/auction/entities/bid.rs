use {
    super::AuctionId,
    crate::{
        kernel::entities::{
            ChainId,
            PermissionKeySvm,
        },
        models::{
            self,
            ProfileId,
        },
        opportunity::entities::OpportunityId,
    },
    express_relay::error::ErrorCode,
    express_relay_api_types::bid as api,
    solana_sdk::{
        clock::Slot,
        instruction::InstructionError,
        pubkey::Pubkey,
        signature::Signature,
        transaction::{
            TransactionError,
            VersionedTransaction,
        },
    },
    std::{
        fmt::{
            Debug,
            Display,
            Formatter,
        },
        sync::Arc,
    },
    strum::FromRepr,
    time::OffsetDateTime,
    tokio::sync::Mutex,
    uuid::Uuid,
};

pub type BidId = Uuid;
pub type BidLock = Arc<Mutex<()>>;

pub trait BidStatus:
    Clone
    + Debug
    + Into<api::BidStatus> // TODO remove this - entity should not depend on api
    + Send
    + Sync
    + PartialEq
{
    type TxHash: Clone + Debug + AsRef<[u8]> + Send + Sync;

    fn convert_tx_hash(tx_hash: &Self::TxHash) -> Vec<u8> {
        tx_hash.as_ref().to_vec()
    }

    fn is_pending(&self) -> bool;
    fn is_awaiting_signature(&self) -> bool;
    fn is_sent_to_user_for_submission(&self) -> bool;
    fn is_submitted(&self) -> bool;
    fn is_cancelled(&self) -> bool;
    fn is_concluded(&self) -> bool;
    fn new_lost() -> Self;

    fn get_auction_id(&self) -> Option<AuctionId>;
}

#[derive(Clone, Debug, PartialEq)]
pub struct BidStatusAuction {
    pub id:      AuctionId,
    pub tx_hash: Signature,
}

#[derive(Clone, Debug, PartialEq)]
pub enum BidSubmissionFailedReason {
    Cancelled,
    DeadlinePassed,
}

#[derive(Clone, Debug, PartialEq, strum::Display)]
#[strum(serialize_all = "snake_case")]
pub enum BidFailedReason {
    InsufficientUserFunds,
    InsufficientSearcherFunds,
    InsufficientFundsSolTransfer,
    DeadlinePassed,
    Other,
}

impl BidFailedReason {
    pub fn get_failed_reason_from_transaction_error(error: &TransactionError) -> Self {
        if let TransactionError::InstructionError(_, InstructionError::Custom(code)) = error {
            return match *code {
                1 => BidFailedReason::InsufficientFundsSolTransfer,
                code if code == u32::from(ErrorCode::DeadlinePassed) => {
                    BidFailedReason::DeadlinePassed
                }
                code if code == u32::from(ErrorCode::InsufficientSearcherFunds) => {
                    BidFailedReason::InsufficientSearcherFunds
                }
                code if code == u32::from(ErrorCode::InsufficientUserFunds) => {
                    BidFailedReason::InsufficientUserFunds
                }
                _ => BidFailedReason::Other,
            };
        }
        BidFailedReason::Other
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum BidStatusSvm {
    Pending,
    AwaitingSignature {
        auction: BidStatusAuction,
    },
    SentToUserForSubmission {
        auction: BidStatusAuction,
    },
    Submitted {
        auction: BidStatusAuction,
    },
    Lost {
        auction: Option<BidStatusAuction>,
    },
    Won {
        auction: BidStatusAuction,
    },
    Failed {
        auction: BidStatusAuction,
        reason:  Option<BidFailedReason>,
    },
    Expired {
        auction: BidStatusAuction,
    },
    Cancelled {
        auction: BidStatusAuction,
    },
    SubmissionFailed {
        auction: BidStatusAuction,
        reason:  BidSubmissionFailedReason,
    },
}

impl BidStatus for BidStatusSvm {
    type TxHash = Signature;

    fn is_pending(&self) -> bool {
        matches!(self, BidStatusSvm::Pending)
    }

    fn is_awaiting_signature(&self) -> bool {
        matches!(self, BidStatusSvm::AwaitingSignature { .. })
    }

    fn is_sent_to_user_for_submission(&self) -> bool {
        matches!(self, BidStatusSvm::SentToUserForSubmission { .. })
    }

    fn is_submitted(&self) -> bool {
        matches!(self, BidStatusSvm::Submitted { .. })
    }

    fn is_cancelled(&self) -> bool {
        matches!(self, BidStatusSvm::Cancelled { .. })
    }

    fn is_concluded(&self) -> bool {
        matches!(
            self,
            BidStatusSvm::Lost { .. }
                | BidStatusSvm::Won { .. }
                | BidStatusSvm::Failed { .. }
                | BidStatusSvm::Expired { .. }
                | BidStatusSvm::Cancelled { .. }
                | BidStatusSvm::SubmissionFailed { .. }
        )
    }

    fn new_lost() -> Self {
        BidStatusSvm::Lost { auction: None }
    }

    fn get_auction_id(&self) -> Option<AuctionId> {
        match self {
            BidStatusSvm::Pending => None,
            BidStatusSvm::AwaitingSignature { auction } => Some(auction.id),
            BidStatusSvm::SentToUserForSubmission { auction } => Some(auction.id),
            BidStatusSvm::Submitted { auction } => Some(auction.id),
            BidStatusSvm::Lost { auction } => auction.as_ref().map(|a| a.id),
            BidStatusSvm::Won { auction } => Some(auction.id),
            BidStatusSvm::Failed { auction, .. } => Some(auction.id),
            BidStatusSvm::Expired { auction } => Some(auction.id),
            BidStatusSvm::Cancelled { auction } => Some(auction.id),
            BidStatusSvm::SubmissionFailed { auction, .. } => Some(auction.id),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Bid {
    pub id:              BidId,
    pub chain_id:        ChainId,
    pub initiation_time: OffsetDateTime,
    pub profile_id:      Option<ProfileId>,

    pub amount:     BidAmountSvm,
    pub status:     BidStatusSvm,
    pub chain_data: BidChainDataSvm,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BidChainDataSvm {
    pub transaction:                  VersionedTransaction,
    pub bid_payment_instruction_type: BidPaymentInstructionType,
    pub router:                       Pubkey,
    pub permission_account:           Pubkey,
}

impl BidChainDataSvm {
    pub fn get_permission_key(&self) -> PermissionKeySvm {
        let mut permission_key = [0; 65];
        permission_key[0] = self.bid_payment_instruction_type.clone().into();
        permission_key[1..33].copy_from_slice(&self.router.to_bytes());
        permission_key[33..].copy_from_slice(&self.permission_account.to_bytes());
        PermissionKeySvm(permission_key)
    }
}

#[derive(Clone, Debug, PartialEq, FromRepr)]
pub enum BidPaymentInstructionType {
    SubmitBid,
    Swap,
}

impl From<BidPaymentInstructionType> for u8 {
    fn from(instruction: BidPaymentInstructionType) -> Self {
        match instruction {
            BidPaymentInstructionType::SubmitBid => 0,
            BidPaymentInstructionType::Swap => 1,
        }
    }
}

impl BidChainDataSvm {
    pub fn get_bid_payment_instruction_type(
        permission_key: &PermissionKeySvm,
    ) -> Option<BidPaymentInstructionType> {
        BidPaymentInstructionType::from_repr(permission_key.0[0].into())
    }

    pub fn get_router(permission_key: &PermissionKeySvm) -> Pubkey {
        let slice: [u8; 32] = permission_key.0[1..33]
            .try_into()
            .expect("Failed to extract bytes 1 through 33 from permission key");
        Pubkey::new_from_array(slice)
    }

    pub fn get_permission_account(permission_key: &PermissionKeySvm) -> Pubkey {
        let slice: [u8; 32] = permission_key.0[33..]
            .try_into()
            .expect("Failed to extract last 32 bytes from permission key");
        Pubkey::new_from_array(slice)
    }
}

#[derive(Clone, Debug)]
pub struct BidCreate {
    pub chain_id:        ChainId,
    pub initiation_time: OffsetDateTime,
    pub profile:         Option<models::Profile>,

    pub chain_data: BidChainDataCreateSvm,
}

#[derive(Clone, Debug)]
pub struct BidChainDataOnChainCreateSvm {
    pub transaction: VersionedTransaction,
    pub slot:        Option<Slot>,
}

#[derive(Clone, Debug)]
pub struct BidChainDataSwapCreateSvm {
    pub transaction:    VersionedTransaction,
    pub opportunity_id: OpportunityId,
}


#[derive(Clone, Debug)]
pub enum BidChainDataCreateSvm {
    OnChain(BidChainDataOnChainCreateSvm),
    Swap(BidChainDataSwapCreateSvm),
}

impl BidChainDataCreateSvm {
    pub fn get_transaction(&self) -> &VersionedTransaction {
        match self {
            BidChainDataCreateSvm::OnChain(data) => &data.transaction,
            BidChainDataCreateSvm::Swap(data) => &data.transaction,
        }
    }
}

pub type BidAmountSvm = u64;

impl PartialEq<Bid> for BidCreate {
    fn eq(&self, other: &Bid) -> bool {
        *self.chain_data.get_transaction() == other.chain_data.transaction
            && self.chain_id == other.chain_id
    }
}

pub struct BidContainerTracing<'a>(pub &'a [Bid]);
impl Display for BidContainerTracing<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:?}",
            self.0
                .iter()
                .map(|x| x.id.to_string())
                .collect::<Vec<String>>()
        )
    }
}
