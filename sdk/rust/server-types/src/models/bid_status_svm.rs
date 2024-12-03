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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum BidStatusSvm {
    #[serde(rename = "Pending")]
    Pending(models::Pending),
    #[serde(rename = "Lost_1")]
    Lost1(models::Lost1),
    #[serde(rename = "Submitted_1")]
    Submitted1(models::Submitted1),
    #[serde(rename = "Won_1")]
    Won1(models::Won1),
    #[serde(rename = "Failed")]
    Failed(models::Failed),
    #[serde(rename = "Expired")]
    Expired(models::Expired),
}

impl Default for BidStatusSvm {
    fn default() -> Self {
        Self::Pending(Default::default())
    }
}
