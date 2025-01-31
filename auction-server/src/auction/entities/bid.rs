use {
    super::AuctionId,
    crate::{
        auction::service::ChainTrait,
        kernel::{
            contracts::MulticallData,
            entities::{
                ChainId,
                Evm,
                PermissionKey as PermissionKeyEvm,
                PermissionKeySvm,
                Svm,
            },
        },
        models::{
            self,
            ProfileId,
        },
        opportunity::entities::OpportunityId,
    },
    ethers::types::{
        Address,
        Bytes,
        H256,
        U256,
    },
    express_relay_api_types::bid as api,
    solana_sdk::{
        clock::Slot,
        pubkey::Pubkey,
        signature::Signature,
        transaction::VersionedTransaction,
    },
    std::{
        fmt::{
            Debug,
            Display,
            Formatter,
        },
        hash::Hash,
    },
    strum::FromRepr,
    time::OffsetDateTime,
    uuid::Uuid,
};

pub type BidId = Uuid;

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
    fn is_submitted(&self) -> bool;
    fn is_finalized(&self) -> bool;

    fn new_lost() -> Self;

    fn get_auction_id(&self) -> Option<AuctionId>;
}

#[derive(Clone, Debug, PartialEq)]
pub struct BidStatusAuction<T: BidStatus> {
    pub id:      AuctionId,
    pub tx_hash: T::TxHash,
}

#[derive(Clone, Debug, PartialEq)]
pub enum BidStatusSvm {
    Pending,
    AwaitingSignature {
        auction: BidStatusAuction<Self>,
    },
    Submitted {
        auction: BidStatusAuction<Self>,
    },
    Lost {
        auction: Option<BidStatusAuction<Self>>,
    },
    Won {
        auction: BidStatusAuction<Self>,
    },
    Failed {
        auction: BidStatusAuction<Self>,
    },
    Expired {
        auction: BidStatusAuction<Self>,
    },
    Cancelled {
        auction: BidStatusAuction<Self>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum BidStatusEvm {
    Pending,
    Submitted {
        auction: BidStatusAuction<Self>,
        index:   u32,
    },
    Lost {
        auction: Option<BidStatusAuction<Self>>,
        index:   Option<u32>,
    },
    Won {
        auction: BidStatusAuction<Self>,
        index:   u32,
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

    fn is_submitted(&self) -> bool {
        matches!(self, BidStatusSvm::Submitted { .. })
    }

    fn is_finalized(&self) -> bool {
        matches!(
            self,
            BidStatusSvm::Lost { .. }
                | BidStatusSvm::Won { .. }
                | BidStatusSvm::Failed { .. }
                | BidStatusSvm::Expired { .. }
                | BidStatusSvm::Cancelled { .. }
        )
    }

    fn new_lost() -> Self {
        BidStatusSvm::Lost { auction: None }
    }

    fn get_auction_id(&self) -> Option<AuctionId> {
        match self {
            BidStatusSvm::Pending => None,
            BidStatusSvm::AwaitingSignature { auction } => Some(auction.id),
            BidStatusSvm::Submitted { auction } => Some(auction.id),
            BidStatusSvm::Lost { auction } => auction.as_ref().map(|a| a.id),
            BidStatusSvm::Won { auction } => Some(auction.id),
            BidStatusSvm::Failed { auction } => Some(auction.id),
            BidStatusSvm::Expired { auction } => Some(auction.id),
            BidStatusSvm::Cancelled { auction } => Some(auction.id),
        }
    }
}

impl BidStatus for BidStatusEvm {
    type TxHash = H256;

    fn is_pending(&self) -> bool {
        matches!(self, BidStatusEvm::Pending)
    }

    fn is_awaiting_signature(&self) -> bool {
        false
    }

    fn is_submitted(&self) -> bool {
        matches!(self, BidStatusEvm::Submitted { .. })
    }

    fn is_finalized(&self) -> bool {
        matches!(self, BidStatusEvm::Lost { .. } | BidStatusEvm::Won { .. })
    }

    fn new_lost() -> Self {
        BidStatusEvm::Lost {
            auction: None,
            index:   None,
        }
    }

    fn get_auction_id(&self) -> Option<AuctionId> {
        match self {
            BidStatusEvm::Pending => None,
            BidStatusEvm::Submitted { auction, .. } => Some(auction.id),
            BidStatusEvm::Lost { auction, .. } => auction.as_ref().map(|a| a.id),
            BidStatusEvm::Won { auction, .. } => Some(auction.id),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Bid<T: ChainTrait> {
    pub id:              BidId,
    pub chain_id:        ChainId,
    pub initiation_time: OffsetDateTime,
    pub profile_id:      Option<ProfileId>,

    pub amount:     T::BidAmountType,
    pub status:     T::BidStatusType,
    pub chain_data: T::BidChainDataType,
}

pub type PermissionKey<T> = <<T as ChainTrait>::BidChainDataType as BidChainData>::PermissionKey;
pub type TxHash<T> = <<T as ChainTrait>::BidStatusType as BidStatus>::TxHash;

pub trait BidChainData: Send + Sync + Clone + Debug + PartialEq {
    type PermissionKey: Send + Sync + Debug + Hash + Eq + Clone + Display;

    fn get_permission_key(&self) -> Self::PermissionKey;
}

#[derive(Clone, Debug, PartialEq)]
pub struct BidChainDataSvm {
    pub transaction:                  VersionedTransaction,
    pub bid_payment_instruction_type: BidPaymentInstructionType,
    pub router:                       Pubkey,
    pub permission_account:           Pubkey,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BidChainDataEvm {
    pub target_contract: Address,
    pub target_calldata: Bytes,
    pub gas_limit:       U256,
    pub permission_key:  Bytes,
}

impl BidChainData for BidChainDataSvm {
    type PermissionKey = PermissionKeySvm;

    fn get_permission_key(&self) -> Self::PermissionKey {
        let mut permission_key = [0; 65];
        permission_key[0] = self.bid_payment_instruction_type.clone().into();
        permission_key[1..33].copy_from_slice(&self.router.to_bytes());
        permission_key[33..].copy_from_slice(&self.permission_account.to_bytes());
        PermissionKeySvm(permission_key)
    }
}

impl BidChainData for BidChainDataEvm {
    type PermissionKey = PermissionKeyEvm;

    fn get_permission_key(&self) -> Self::PermissionKey {
        self.permission_key.clone()
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
pub struct BidCreate<T: ChainTrait> {
    pub chain_id:        ChainId,
    pub initiation_time: OffsetDateTime,
    pub profile:         Option<models::Profile>,

    pub chain_data: T::BidChainDataCreateType,
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

#[derive(Clone, Debug)]
pub struct BidChainDataCreateEvm {
    pub target_contract: Address,
    pub target_calldata: Bytes,
    pub permission_key:  Bytes,
    pub amount:          U256,
}

pub type BidAmountSvm = u64;
pub type BidAmountEvm = U256;

impl PartialEq<Bid<Svm>> for BidCreate<Svm> {
    fn eq(&self, other: &Bid<Svm>) -> bool {
        *self.chain_data.get_transaction() == other.chain_data.transaction
            && self.chain_id == other.chain_id
    }
}

impl From<(Bid<Evm>, bool)> for MulticallData {
    fn from((bid, revert_on_failure): (Bid<Evm>, bool)) -> Self {
        MulticallData {
            bid_id: bid.id.into_bytes(),
            target_contract: bid.chain_data.target_contract,
            target_calldata: bid.chain_data.target_calldata,
            bid_amount: bid.amount,
            gas_limit: bid.chain_data.gas_limit,
            revert_on_failure,
        }
    }
}

pub struct BidContainerTracing<'a, T: ChainTrait>(pub &'a [Bid<T>]);
impl<T: ChainTrait> Display for BidContainerTracing<'_, T> {
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
