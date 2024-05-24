use {
    email_address,
    ethers::types::H256,
    sqlx::{
        prelude::FromRow,
        types::{
            time::PrimitiveDateTime,
            BigDecimal,
        },
    },
    std::{
        ops::Deref,
        str::FromStr,
    },
    uuid::Uuid,
};

#[derive(Clone)]
pub struct NullableH256(pub Option<H256>);

impl From<Option<Vec<u8>>> for NullableH256 {
    fn from(value: Option<Vec<u8>>) -> Self {
        match value {
            Some(value) => NullableH256(Some(H256::from_slice(value.as_slice()))),
            None => NullableH256(None),
        }
    }
}

impl Deref for NullableH256 {
    type Target = Option<H256>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub type AuctionId = Uuid;
#[derive(Clone, FromRow)]
pub struct Auction {
    pub id:                  AuctionId,
    pub creation_time:       PrimitiveDateTime,
    pub conclusion_time:     Option<PrimitiveDateTime>,
    pub permission_key:      Vec<u8>,
    pub chain_id:            String,
    #[sqlx(try_from = "Option<Vec<u8>>")]
    pub tx_hash:             NullableH256,
    pub bid_collection_time: Option<PrimitiveDateTime>,
    pub submission_time:     Option<PrimitiveDateTime>,
}


#[derive(Clone)]
pub struct WrappedEmailAddress {
    pub value: email_address::EmailAddress,
}

impl WrappedEmailAddress {
    pub fn new(value: email_address::EmailAddress) -> Self {
        WrappedEmailAddress { value }
    }
}

impl TryFrom<String> for WrappedEmailAddress {
    type Error = email_address::Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let value = email_address::EmailAddress::from_str(value.as_str())?;
        Ok(WrappedEmailAddress::new(value))
    }
}

pub type ProfileId = Uuid;
#[derive(Clone, FromRow)]
pub struct Profile {
    pub id:    ProfileId,
    pub name:  String,
    #[sqlx(try_from = "String")]
    pub email: WrappedEmailAddress,

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

#[derive(Clone, Debug)]
pub struct BundleIndex(pub Option<u32>);

impl TryFrom<Option<i32>> for BundleIndex {
    type Error = anyhow::Error;
    fn try_from(value: Option<i32>) -> Result<Self, Self::Error> {
        match value {
            Some(value) => {
                let result: u32 = value.try_into()?;
                Ok(BundleIndex(Some(result)))
            }
            None => Ok(BundleIndex(None)),
        }
    }
}

impl Deref for BundleIndex {
    type Target = Option<u32>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Clone, FromRow, Debug)]
pub struct Bid {
    pub id:              BidId,
    pub creation_time:   PrimitiveDateTime,
    pub permission_key:  Vec<u8>,
    pub chain_id:        String,
    pub target_contract: Vec<u8>,
    pub target_calldata: Vec<u8>,
    pub bid_amount:      BigDecimal,
    pub status:          BidStatus,
    pub auction_id:      Option<AuctionId>,
    #[sqlx(try_from = "Option<i32>")]
    pub bundle_index:    BundleIndex,
    pub initiation_time: PrimitiveDateTime,
    pub profile_id:      Option<ProfileId>,
}

impl Bid {
    pub fn is_for_auction(&self, auction: &Option<Auction>) -> bool {
        match auction {
            Some(a) => self.auction_id == Some(a.id),
            None => self.auction_id.is_none(),
        }
    }
}
