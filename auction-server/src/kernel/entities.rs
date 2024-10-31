use {
    ethers::types::Bytes,
    serde::{
        Deserialize,
        Serialize,
    },
    serde_with::{
        base64::{
            Base64,
            Standard,
        },
        formats::Padded,
        DeserializeAs,
        SerializeAs,
    },
};

pub type ChainId = String;
pub type PermissionKey = Bytes;

#[derive(Clone, Debug)]
pub struct PermissionKeySvm(pub [u8; 64]);

impl Serialize for PermissionKeySvm {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        Base64::<Standard, Padded>::serialize_as(&self.0, serializer)
    }
}

impl<'de> Deserialize<'de> for PermissionKeySvm {
    fn deserialize<D>(deserializer: D) -> Result<PermissionKeySvm, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let bytes = Base64::<Standard, Padded>::deserialize_as(deserializer)?;
        Ok(PermissionKeySvm(bytes))
    }
}
