use {
    crate::api::RestError,
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
        pubkey::Pubkey,
        signature::Signature,
        transaction::VersionedTransaction,
    },
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

#[derive(Clone, Debug)]
pub struct Evm;

#[derive(Clone, Debug)]
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

    pub fn all_signatures_exists(
        message_bytes: &[u8],
        accounts: &[Pubkey],
        signatures: &[Signature],
        missing_signers: &[Pubkey],
    ) -> bool {
        signatures
            .iter()
            .zip(accounts.iter())
            .all(|(signature, pubkey)| {
                signature.verify(pubkey.as_ref(), message_bytes) || missing_signers.contains(pubkey)
            })
    }
}
