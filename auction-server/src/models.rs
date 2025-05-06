use {
    sqlx::{
        prelude::FromRow,
        types::time::PrimitiveDateTime,
    },
    std::str::FromStr,
    uuid::Uuid,
};

#[derive(Clone, Debug, PartialEq)]
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

#[derive(Clone, FromRow, Debug, PartialEq)]
pub struct Profile {
    pub id:    ProfileId,
    pub name:  String,
    #[sqlx(try_from = "String")]
    pub email: EmailAddress,
    pub role:  ProfileRole,

    #[allow(dead_code)]
    pub created_at: PrimitiveDateTime,
    #[allow(dead_code)]
    pub updated_at: PrimitiveDateTime,
}

pub type TokenId = Uuid;
pub type AccessTokenToken = String;
#[derive(Clone)]
#[allow(dead_code)]
pub struct AccessToken {
    pub id: TokenId,

    pub token:      AccessTokenToken,
    pub profile_id: ProfileId,
    pub revoked_at: Option<PrimitiveDateTime>,

    pub created_at: PrimitiveDateTime,
    pub updated_at: PrimitiveDateTime,
}

#[derive(Clone, Debug, PartialEq, PartialOrd, sqlx::Type)]
#[sqlx(type_name = "chain_type", rename_all = "lowercase")]
pub enum ChainType {
    Evm,
    Svm,
}
