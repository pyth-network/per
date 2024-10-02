use {
    ethers::types::{
        Address,
        Bytes,
    },
    serde::{
        Deserialize,
        Serialize,
    },
    solana_sdk::transaction::VersionedTransaction,
    sqlx::{
        prelude::FromRow,
        types::{
            time::PrimitiveDateTime,
            BigDecimal,
            Json,
        },
    },
    std::{
        ops::Deref,
        str::FromStr,
    },
    uuid::Uuid,
};

pub type AuctionId = Uuid;
#[derive(Clone, FromRow, Debug)]
pub struct Auction {
    pub id:                  AuctionId,
    pub creation_time:       PrimitiveDateTime,
    pub conclusion_time:     Option<PrimitiveDateTime>,
    pub permission_key:      Vec<u8>,
    pub chain_id:            String,
    pub chain_type:          ChainType,
    pub tx_hash:             Option<Vec<u8>>,
    pub bid_collection_time: Option<PrimitiveDateTime>,
    pub submission_time:     Option<PrimitiveDateTime>,
}

#[derive(Clone)]
pub struct EmailAddress(pub email_address::EmailAddress);

impl TryFrom<String> for EmailAddress {
    type Error = email_address::Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let value = email_address::EmailAddress::from_str(value.as_str())?;
        Ok(EmailAddress(value))
    }
}

pub type ProfileId = Uuid;
#[derive(Clone, Debug, sqlx::Type, PartialEq, PartialOrd)]
#[sqlx(type_name = "profile_role", rename_all = "lowercase")]
pub enum ProfileRole {
    Searcher,
    Protocol,
}

#[derive(Clone, FromRow)]
pub struct Profile {
    pub id:    ProfileId,
    pub name:  String,
    #[sqlx(try_from = "String")]
    pub email: EmailAddress,
    pub role:  ProfileRole,

    pub created_at: PrimitiveDateTime,
    pub updated_at: PrimitiveDateTime,
}

pub type TokenId = Uuid;
pub type AccessTokenToken = String;
#[derive(Clone)]
pub struct AccessToken {
    pub id: TokenId,

    pub token:      AccessTokenToken,
    pub profile_id: ProfileId,
    pub revoked_at: Option<PrimitiveDateTime>,

    pub created_at: PrimitiveDateTime,
    pub updated_at: PrimitiveDateTime,
}


pub type BidId = Uuid;
#[derive(Clone, Debug, PartialEq, PartialOrd, sqlx::Type)]
#[sqlx(type_name = "bid_status", rename_all = "lowercase")]
pub enum BidStatus {
    Pending,
    Submitted,
    Lost,
    Won,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BundleIndex(pub Option<u32>);
impl Deref for BundleIndex {
    type Target = Option<u32>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, PartialOrd, sqlx::Type)]
#[sqlx(type_name = "chain_type", rename_all = "lowercase")]
pub enum ChainType {
    Evm,
    Svm,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BidMetadataEvm {
    pub target_contract: Address,
    pub target_calldata: Bytes,
    pub bundle_index:    BundleIndex,
    pub gas_limit:       u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BidMetadataSvm {
    #[serde(with = "crate::serde::transaction_svm")]
    pub transaction: VersionedTransaction,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BidMetadata {
    Evm(BidMetadataEvm),
    Svm(BidMetadataSvm),
}

#[derive(Clone, Debug, FromRow)]
pub struct Bid {
    pub id:              BidId,
    pub creation_time:   PrimitiveDateTime,
    pub permission_key:  Vec<u8>,
    pub chain_id:        String,
    pub chain_type:      ChainType,
    pub bid_amount:      BigDecimal,
    pub status:          BidStatus,
    pub auction_id:      Option<AuctionId>,
    pub initiation_time: PrimitiveDateTime,
    pub profile_id:      Option<ProfileId>,
    pub metadata:        Json<BidMetadata>,
}

impl Bid {
    pub fn is_for_auction(&self, auction: &Option<Auction>) -> bool {
        match auction {
            Some(a) => self.auction_id == Some(a.id),
            None => self.auction_id.is_none(),
        }
    }
}

impl BidMetadata {
    pub fn get_bundle_index(&self) -> Option<u32> {
        match self {
            BidMetadata::Evm(metadata) => *metadata.bundle_index,
            BidMetadata::Svm(_) => None,
        }
    }
}

impl TryInto<BidMetadataEvm> for BidMetadata {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<BidMetadataEvm, Self::Error> {
        match self {
            BidMetadata::Evm(metadata) => Ok(metadata),
            _ => Err(anyhow::anyhow!("Invalid metadata type")),
        }
    }
}

impl TryInto<BidMetadataSvm> for BidMetadata {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<BidMetadataSvm, Self::Error> {
        match self {
            BidMetadata::Svm(metadata) => Ok(metadata),
            _ => Err(anyhow::anyhow!("Invalid metadata type")),
        }
    }
}
