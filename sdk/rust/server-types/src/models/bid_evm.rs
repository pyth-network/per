/*
 * auction-server
 *
 * No description provided (generated by Openapi Generator https://github.com/openapitools/openapi-generator)
 *
 * The version of the OpenAPI document: 0.14.0
 *
 * Generated by: https://openapi-generator.tech
 */

use {
    crate::models,
    serde::{
        Deserialize,
        Serialize,
    },
};

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct BidEvm {
    /// The chain id for bid.
    #[serde(rename = "chain_id")]
    pub chain_id:        String,
    /// The unique id for bid.
    #[serde(rename = "id")]
    pub id:              String,
    /// The time server received the bid formatted in rfc3339.
    #[serde(rename = "initiation_time")]
    pub initiation_time: String,
    /// The profile id for the bid owner.
    #[serde(rename = "profile_id")]
    pub profile_id:      String,
    /// Amount of bid in wei.
    #[serde(rename = "bid_amount")]
    pub bid_amount:      String,
    /// The gas limit for the contract call.
    #[serde(rename = "gas_limit")]
    pub gas_limit:       String,
    /// The permission key for bid.
    #[serde(rename = "permission_key")]
    pub permission_key:  String,
    #[serde(rename = "status")]
    pub status:          models::BidStatusEvm,
    /// Calldata for the contract call.
    #[serde(rename = "target_calldata")]
    pub target_calldata: String,
    /// The contract address to call.
    #[serde(rename = "target_contract")]
    pub target_contract: String,
}

impl BidEvm {
    pub fn new(
        chain_id: String,
        id: String,
        initiation_time: String,
        profile_id: String,
        bid_amount: String,
        gas_limit: String,
        permission_key: String,
        status: models::BidStatusEvm,
        target_calldata: String,
        target_contract: String,
    ) -> BidEvm {
        BidEvm {
            chain_id,
            id,
            initiation_time,
            profile_id,
            bid_amount,
            gas_limit,
            permission_key,
            status,
            target_calldata,
            target_contract,
        }
    }
}
