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
pub struct BidStatusWithId {
    #[serde(rename = "bid_status")]
    pub bid_status: models::BidStatus,
    #[serde(rename = "id")]
    pub id:         String,
}

impl BidStatusWithId {
    pub fn new(bid_status: models::BidStatus, id: String) -> BidStatusWithId {
        BidStatusWithId { bid_status, id }
    }
}
