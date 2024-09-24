use {
    crate::{
        kernel::entities::PermissionKey,
        opportunity::entities::{
            opportunity::OpportunityCoreFields,
            opportunity_evm::OpportunityEvm,
        },
    },
    ethers::types::Bytes,
    sqlx::{
        prelude::FromRow,
        types::{
            time::PrimitiveDateTime,
            BigDecimal,
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

#[derive(Clone, FromRow, Debug)]
pub struct Opportunity {
    pub id:                Uuid,
    pub creation_time:     PrimitiveDateTime,
    pub permission_key:    Vec<u8>,
    pub chain_id:          String,
    pub target_contract:   Vec<u8>,
    pub target_call_value: BigDecimal,
    pub target_calldata:   Vec<u8>,
    pub removal_time:      Option<PrimitiveDateTime>,
    pub sell_tokens:       JsonValue,
    pub buy_tokens:        JsonValue,
    pub removal_reason:    Option<OpportunityRemovalReason>,
}

impl TryFrom<Opportunity> for OpportunityEvm {
    type Error = anyhow::Error;

    fn try_from(val: Opportunity) -> Result<Self, Self::Error> {
        Ok(OpportunityEvm {
            core_fields:       OpportunityCoreFields {
                id:             val.id,
                creation_time:  val.creation_time.assume_utc().unix_timestamp_nanos(),
                permission_key: PermissionKey::from(val.permission_key.clone()),
                chain_id:       val.chain_id,
                sell_tokens:    serde_json::from_value(val.sell_tokens)
                    .map_err(|e| anyhow::anyhow!(e))?,
                buy_tokens:     serde_json::from_value(val.buy_tokens)
                    .map_err(|e| anyhow::anyhow!(e))?,
            },
            target_contract:   ethers::abi::Address::from_slice(&val.target_contract),
            target_call_value: val.target_call_value.to_string().parse().unwrap(),
            target_calldata:   Bytes::from(val.target_calldata),
        })
    }
}
