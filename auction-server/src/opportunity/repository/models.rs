use {
    crate::models::ChainType,
    ethers::types::{
        Address,
        Bytes,
        U256,
    },
    serde::{
        de::DeserializeOwned,
        Deserialize,
        Serialize,
    },
    serde_with::{
        base64::Base64,
        serde_as,
        DisplayFromStr,
    },
    solana_sdk::{
        hash::Hash,
        pubkey::Pubkey,
    },
    sqlx::{
        prelude::FromRow,
        types::{
            time::PrimitiveDateTime,
            Json,
            JsonValue,
        },
    },
    uuid::Uuid,
};

#[derive(Clone, Debug, PartialEq, PartialOrd, sqlx::Type)]
#[sqlx(type_name = "opportunity_removal_reason", rename_all = "lowercase")]
pub enum OpportunityRemovalReason {
    Expired,
    Invalid,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OpportunityMetadataEvm {
    pub target_contract:   Address,
    #[serde(with = "crate::serde::u256")]
    pub target_call_value: U256,
    pub target_calldata:   Bytes,
}

#[serde_as]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OpportunityMetadataSvmClientKamino {
    #[serde_as(as = "Base64")]
    pub order: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "name", rename_all = "lowercase")]
pub enum OpportunityMetadataSvmClient {
    Kamino(OpportunityMetadataSvmClientKamino),
}

#[serde_as]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OpportunityMetadataSvm {
    pub client:     OpportunityMetadataSvmClient,
    #[serde_as(as = "DisplayFromStr")]
    pub router:     Pubkey,
    #[serde_as(as = "DisplayFromStr")]
    pub permission: Pubkey,
    #[serde_as(as = "DisplayFromStr")]
    pub block_hash: Hash,
}

pub trait OpportunityMetadata: Serialize + DeserializeOwned {
    fn get_chain_type() -> ChainType;
}

impl OpportunityMetadata for OpportunityMetadataEvm {
    fn get_chain_type() -> ChainType {
        ChainType::Evm
    }
}

impl OpportunityMetadata for OpportunityMetadataSvm {
    fn get_chain_type() -> ChainType {
        ChainType::Svm
    }
}

#[derive(Clone, FromRow, Debug)]
pub struct Opportunity<T: OpportunityMetadata> {
    pub id:             Uuid,
    pub creation_time:  PrimitiveDateTime,
    pub permission_key: Vec<u8>,
    pub chain_id:       String,
    pub chain_type:     ChainType,
    pub removal_time:   Option<PrimitiveDateTime>,
    pub sell_tokens:    JsonValue,
    pub buy_tokens:     JsonValue,
    pub removal_reason: Option<OpportunityRemovalReason>,
    pub metadata:       Json<T>,
}

// Add blockhash
// remove tokens from svm opps
// Move impl
