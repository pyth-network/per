use {
    ::serde::{
        Deserialize,
        Serialize,
    },
    serde_with::{
        base64::{
            Base64,
            Standard,
        },
        formats::Padded,
        serde_as,
        DeserializeAs,
        DisplayFromStr,
        SerializeAs,
    },
    solana_sdk::hash::Hash,
    strum::AsRefStr,
    utoipa::{
        ToResponse,
        ToSchema,
    },
};

pub mod bid;
pub mod opportunity;
pub mod profile;
pub mod quote;
pub mod serde;
pub mod ws;

pub type MicroLamports = u64;
pub type ChainId = String;

#[derive(Clone, Debug)]
pub struct PermissionKeySvm(pub [u8; 65]);
impl Serialize for PermissionKeySvm {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ::serde::Serializer,
    {
        Base64::<Standard, Padded>::serialize_as(&self.0, serializer)
    }
}

impl<'de> Deserialize<'de> for PermissionKeySvm {
    fn deserialize<D>(deserializer: D) -> Result<PermissionKeySvm, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        let bytes = Base64::<Standard, Padded>::deserialize_as(deserializer)?;
        Ok(PermissionKeySvm(bytes))
    }
}

#[serde_as]
#[derive(Serialize, Clone, ToSchema, ToResponse, Deserialize, Debug, PartialEq)]
pub struct SvmChainUpdate {
    #[schema(example = "solana", value_type = String)]
    pub chain_id:                  ChainId,
    #[serde_as(as = "DisplayFromStr")]
    #[schema(example = "SLxp9LxX1eE9Z5v99Y92DaYEwyukFgMUF6zRerCF12j", value_type = String)]
    pub blockhash:                 Hash,
    /// The prioritization fee that the server suggests to use for the next transaction
    #[schema(example = "1000", value_type = u64)]
    pub latest_prioritization_fee: MicroLamports,
}

#[derive(ToResponse, ToSchema, Serialize, Deserialize)]
#[response(description = "An error occurred processing the request")]
pub struct ErrorBodyResponse {
    pub error: String,
}

#[derive(AsRefStr)]
#[strum(prefix = "/")]
pub enum Route {
    #[strum(serialize = "v1")]
    V1,
    #[strum(serialize = "v1/:chain_id")]
    V1Chain,
    #[strum(serialize = "bids")]
    Bid,
    #[strum(serialize = "opportunities")]
    Opportunity,
    #[strum(serialize = "quotes")]
    Quote,
    #[strum(serialize = "profiles")]
    Profile,
    #[strum(serialize = "")]
    Root,
    #[strum(serialize = "live")]
    Liveness,
    #[strum(serialize = "docs")]
    Docs,
    #[strum(serialize = "docs/openapi.json")]
    OpenApi,
}

#[derive(PartialEq)]
pub enum AccessLevel {
    Admin,
    LoggedIn,
    Public,
}

pub struct RouteProperties {
    pub access_level: AccessLevel,
    pub method:       http::Method,
    pub full_path:    String,
}

pub trait Routable: AsRef<str> + Clone {
    fn properties(&self) -> RouteProperties;
}
