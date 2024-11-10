#![allow(dead_code)]

use {
    crate::{
        kernel::entities::{
            ChainId,
            Evm,
            PermissionKeySvm,
            Svm,
        },
        models::{
            self,
            ProfileId,
        },
    },
    ethers::types::{
        Address,
        Bytes,
        H256,
        U256,
    },
    solana_sdk::{
        pubkey::Pubkey,
        signature::Signature,
        transaction::VersionedTransaction,
    },
    time::OffsetDateTime,
    uuid::Uuid,
};

pub type BidId = Uuid;

pub trait BidStatus:
    Clone
    + std::fmt::Debug
    // + PartialEq<models::BidStatus>
    // + Into<BidStatus>
    // + Into<models::BidStatus>
    + Send
    + Sync
{
    type TxHash: Clone + std::fmt::Debug + AsRef<[u8]> + Send + Sync;

    // fn get_update_query(
    //     &self,
    //     id: BidId,
    //     auction: Option<&models::Auction>,
    // ) -> anyhow::Result<Query<'_, Postgres, PgArguments>>;
    // fn extract_by(bid: models::Bid, auction: Option<models::Auction>) -> anyhow::Result<Self>;
    fn convert_tx_hash(tx_hash: &Self::TxHash) -> Vec<u8> {
        tx_hash.as_ref().to_vec()
    }
    fn get_tx_hash(&self) -> Option<&Self::TxHash>;
}

#[derive(Clone, Debug)]
pub enum BidStatusSvm {
    Pending,
    Submitted { signature: Signature },
    Lost { signature: Option<Signature> },
    Won { signature: Signature },
    Expired { signature: Signature },
}

#[derive(Clone, Debug)]
pub enum BidStatusEvm {
    Pending,
    Submitted {
        tx_hash: H256,
        index:   u32,
    },
    Lost {
        tx_hash: Option<H256>,
        index:   Option<u32>,
    },
    Won {
        tx_hash: H256,
        index:   u32,
    },
}

impl BidStatus for BidStatusSvm {
    type TxHash = Signature;

    fn get_tx_hash(&self) -> Option<&Self::TxHash> {
        match self {
            BidStatusSvm::Pending => None,
            BidStatusSvm::Submitted { signature } => Some(signature),
            BidStatusSvm::Lost { signature } => signature.as_ref(),
            BidStatusSvm::Won { signature } => Some(signature),
            BidStatusSvm::Expired { signature } => Some(signature),
        }
    }
}

impl BidStatus for BidStatusEvm {
    type TxHash = H256;

    fn get_tx_hash(&self) -> Option<&Self::TxHash> {
        match self {
            BidStatusEvm::Pending => None,
            BidStatusEvm::Submitted { tx_hash, .. } => Some(tx_hash),
            BidStatusEvm::Lost { tx_hash, .. } => tx_hash.as_ref(),
            BidStatusEvm::Won { tx_hash, .. } => Some(tx_hash),
        }
    }
}

pub trait BidTrait:
    Clone
    // + Into<api::Bid>
    + std::fmt::Debug
    // + TryFrom<(models::Bid, Option<models::Auction>)>
    // + Deref<Target = SimulatedBidCoreFields>
    + Send
    + Sync
{
    type StatusType: BidStatus;
    type ChainData: BidChainData;
    type BidAmount: std::fmt::Debug;

    // fn update_status(self, status: Self::StatusType) -> Self;
    // fn get_metadata(&self) -> anyhow::Result<models::BidMetadata>;
    // fn get_chain_type(&self) -> models::ChainType;
    // fn get_bid_amount_string(&self) -> String;
    // fn get_permission_key(&self) -> &[u8];
    // fn get_permission_key_as_bytes(&self) -> Bytes {
    //     Bytes::from(self.get_permission_key().to_vec())
    // }
    // fn get_bid_status(
    //     status: models::BidStatus,
    //     index: Option<u32>,
    //     result: Option<<Self::StatusType as BidStatus>::TxHash>,
    // ) -> anyhow::Result<Self::StatusType>;
}

#[derive(Clone, Debug)]
pub struct Bid<T: BidTrait> {
    pub id:              BidId,
    pub chain_id:        ChainId,
    pub initiation_time: OffsetDateTime,
    pub profile_id:      Option<ProfileId>,

    pub amount:     T::BidAmount,
    pub status:     T::StatusType,
    pub chain_data: T::ChainData,
}

pub trait BidChainData: std::fmt::Debug {
    type PermissionKey: AsRef<[u8]> + std::fmt::Debug;

    fn get_permission_key(&self) -> Self::PermissionKey;
}

#[derive(Clone, Debug)]
pub struct BidChainDataSvm {
    pub transaction:        VersionedTransaction,
    pub router:             Pubkey,
    pub permission_account: Pubkey,
}

#[derive(Clone, Debug)]
pub struct BidChainDataEvm {
    pub target_contract: Address,
    pub target_calldata: Bytes,
    pub gas_limit:       U256,
    pub permission_key:  Bytes,
}

impl BidChainData for BidChainDataSvm {
    type PermissionKey = [u8; 64];

    fn get_permission_key(&self) -> Self::PermissionKey {
        let mut permission_key = [0; 64];
        permission_key[..32].copy_from_slice(&self.router.to_bytes());
        permission_key[32..].copy_from_slice(&self.permission_account.to_bytes());
        permission_key
    }
}

impl BidChainData for BidChainDataEvm {
    type PermissionKey = Bytes;

    fn get_permission_key(&self) -> Self::PermissionKey {
        self.permission_key.clone()
    }
}

impl BidTrait for Evm {
    type StatusType = BidStatusEvm;
    type ChainData = BidChainDataEvm;
    type BidAmount = U256;
}

impl BidTrait for Svm {
    type StatusType = BidStatusSvm;
    type ChainData = BidChainDataSvm;
    type BidAmount = u64;
}

impl BidChainDataSvm {
    pub fn get_router(permission_key: &PermissionKeySvm) -> Pubkey {
        let slice: [u8; 32] = permission_key.0[..32]
            .try_into()
            .expect("Failed to extract first 32 bytes from permission key");
        Pubkey::new_from_array(slice)
    }

    pub fn get_permission_account(permission_key: &PermissionKeySvm) -> Pubkey {
        let slice: [u8; 32] = permission_key.0[32..]
            .try_into()
            .expect("Failed to extract last 32 bytes from permission key");
        Pubkey::new_from_array(slice)
    }
}


pub trait BidCreateTrait: Clone + std::fmt::Debug {
    type ChainDataCreate: Clone + std::fmt::Debug;
}

#[derive(Clone, Debug)]
pub struct BidCreate<T: BidCreateTrait> {
    pub chain_id:        ChainId,
    pub initiation_time: OffsetDateTime,
    pub profile:         Option<models::Profile>,

    pub chain_data: T::ChainDataCreate,
}

impl BidCreateTrait for Evm {
    type ChainDataCreate = BidChainDataCreateEvm;
}

impl BidCreateTrait for Svm {
    type ChainDataCreate = BidChainDataCreateSvm;
}

#[derive(Clone, Debug)]
pub struct BidChainDataCreateSvm {
    pub transaction: VersionedTransaction,
}

#[derive(Clone, Debug)]
pub struct BidChainDataCreateEvm {
    pub target_contract: Address,
    pub target_calldata: Bytes,
    pub permission_key:  Bytes,
    pub amount:          U256,
}
