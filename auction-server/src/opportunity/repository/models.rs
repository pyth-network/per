use {
    crate::{
        kernel::entities::PermissionKey,
        models::ChainType,
        opportunity::entities,
    },
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
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OpportunityMetadataSvm {
    #[serde_as(as = "Base64")]
    pub order:      Vec<u8>,
    pub router:     Pubkey,
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

impl TryFrom<Opportunity<OpportunityMetadataEvm>> for entities::OpportunityEvm {
    type Error = anyhow::Error;

    fn try_from(val: Opportunity<OpportunityMetadataEvm>) -> Result<Self, Self::Error> {
        Ok(entities::OpportunityEvm {
            core_fields:       entities::OpportunityCoreFields {
                id:             val.id,
                creation_time:  val.creation_time.assume_utc().unix_timestamp_nanos(),
                permission_key: PermissionKey::from(val.permission_key),
                chain_id:       val.chain_id,
                sell_tokens:    serde_json::from_value(val.sell_tokens)
                    .map_err(|e| anyhow::anyhow!(e))?,
                buy_tokens:     serde_json::from_value(val.buy_tokens)
                    .map_err(|e| anyhow::anyhow!(e))?,
            },
            target_contract:   val.metadata.target_contract,
            target_call_value: val.metadata.target_call_value,
            target_calldata:   val.metadata.target_calldata.clone(),
        })
    }
}

// Add blockhash
// remove tokens from svm opps
// Move impl
