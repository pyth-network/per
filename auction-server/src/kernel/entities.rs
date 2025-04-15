use {
    crate::api::RestError,
    base64::Engine,
    bincode::serialized_size,
    solana_sdk::{
        packet::PACKET_DATA_SIZE,
        transaction::VersionedTransaction,
    },
    std::{
        array::TryFromSliceError,
        fmt::Display,
    },
};

pub type ChainId = String;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PermissionKeySvm(pub [u8; 65]);
impl Display for PermissionKeySvm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            base64::engine::general_purpose::STANDARD.encode(self.0)
        )
    }
}

impl PermissionKeySvm {
    pub fn to_vec(&self) -> Vec<u8> {
        self.0.to_vec()
    }
}

impl AsRef<[u8]> for PermissionKeySvm {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl TryFrom<&[u8]> for PermissionKeySvm {
    type Error = TryFromSliceError;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        <[u8; 65]>::try_from(value).map(Self)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Svm;

impl Svm {
    pub fn check_tx_size(transaction: &VersionedTransaction) -> Result<(), RestError> {
        let size = serialized_size(&transaction).map_err(|e| {
            RestError::BadParameters(format!("Error serializing transaction: {:?}", e))
        })?;
        if size > PACKET_DATA_SIZE as u64 {
            return Err(RestError::TransactionSizeTooLarge(size, PACKET_DATA_SIZE));
        }
        Ok(())
    }
}
