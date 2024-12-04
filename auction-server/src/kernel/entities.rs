use {
    crate::api::RestError,
    base64::Engine,
    bincode::serialized_size,
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
    solana_sdk::{
        packet::PACKET_DATA_SIZE,
        transaction::VersionedTransaction,
    },
    std::fmt::Display,
};

pub type ChainId = String;
pub type PermissionKey = Bytes;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PermissionKeySvm(pub [u8; 64]);

impl Serialize for PermissionKeySvm {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        Base64::<Standard, Padded>::serialize_as(&self.0, serializer)
    }
}

impl Display for PermissionKeySvm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            base64::engine::general_purpose::STANDARD.encode(self.0)
        )
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

#[derive(Debug, PartialEq, Clone)]
pub enum ChainType {
    Evm,
    Svm,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Evm;

#[derive(Clone, Debug, PartialEq)]
pub struct Svm;

impl Svm {
    pub fn check_tx_size(transaction: &VersionedTransaction) -> Result<(), RestError> {
        let size = serialized_size(&transaction).map_err(|e| {
            RestError::BadParameters(format!("Error serializing transaction: {:?}", e))
        })?;
        if size > PACKET_DATA_SIZE as u64 {
            return Err(RestError::BadParameters(format!(
                "Transaction size is too large: {} > {}",
                size, PACKET_DATA_SIZE
            )));
        }
        Ok(())
    }
}
