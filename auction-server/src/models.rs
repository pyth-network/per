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

pub type PermissionId = Uuid;

#[derive(Clone, Debug, PartialEq, PartialOrd, Hash, Eq, sqlx::Type)]
#[sqlx(rename_all = "snake_case")]
pub enum PermissionFeature {
    CancelQuote,
}

impl TryFrom<String> for PermissionFeature {
    type Error = sqlx::error::BoxDynError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "cancel_quote" => Ok(PermissionFeature::CancelQuote),
            _ => Err(sqlx::error::BoxDynError::from("Invalid permission feature")),
        }
    }
}

impl PermissionFeature {
    pub fn as_str(&self) -> &'static str {
        match self {
            PermissionFeature::CancelQuote => "cancel_quote",
        }
    }
}

#[derive(Clone, Debug, sqlx::Type, PartialEq, PartialOrd)]
#[sqlx(type_name = "permission_state", rename_all = "snake_case")]
pub enum PermissionState {
    Enabled,
    Disabled,
}

#[derive(Clone, FromRow, Debug, PartialEq)]
pub struct Permission {
    pub id: PermissionId,

    #[sqlx(try_from = "String")]
    pub feature:    PermissionFeature,
    pub profile_id: ProfileId,
    pub state:      PermissionState,

    pub created_at: PrimitiveDateTime,
    pub updated_at: PrimitiveDateTime,
}
