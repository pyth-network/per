use {
    ::serde::{
        Deserialize,
        Serialize,
    },
    ethers::types::Bytes,
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
    utoipa::{
        ToResponse,
        ToSchema,
    },
};

pub mod bid;
pub mod opportunity;
pub mod profile;
pub mod serde;

pub type MicroLamports = u64;
pub type ChainId = String;
pub type PermissionKeyEvm = Bytes;
#[derive(Clone, Debug)]
pub struct PermissionKeySvm(pub [u8; 64]);
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
#[derive(Serialize, Clone, ToSchema, ToResponse)]
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

#[derive(ToResponse, ToSchema, Serialize)]
#[response(description = "An error occurred processing the request")]
pub struct ErrorBodyResponse {
    pub error: String,
}
