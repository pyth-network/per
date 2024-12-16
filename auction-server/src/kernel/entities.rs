use {
    crate::api::RestError,
    base64::Engine,
    bincode::serialized_size,
    ethers::types::Bytes,
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
impl Display for PermissionKeySvm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            base64::engine::general_purpose::STANDARD.encode(self.0)
        )
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
