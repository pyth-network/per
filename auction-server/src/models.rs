use {
    email_address,
    ethers::types::H256,
    sqlx::{
        prelude::FromRow,
        types::time::PrimitiveDateTime,
    },
    std::str::FromStr,
    uuid::Uuid,
};

#[derive(Clone)]
pub struct Auction {
    pub id:                  Uuid,
    pub creation_time:       PrimitiveDateTime,
    pub conclusion_time:     Option<PrimitiveDateTime>,
    pub permission_key:      Vec<u8>,
    pub chain_id:            String,
    pub tx_hash:             Option<H256>,
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
#[derive(Clone)]
pub struct AccessToken {
    pub id: TokenId,

    pub token:      String,
    pub profile_id: ProfileId,
    pub revoked_at: Option<PrimitiveDateTime>,

    pub created_at: PrimitiveDateTime,
    pub updated_at: PrimitiveDateTime,
}
