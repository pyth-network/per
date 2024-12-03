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

/// Expired : The bid was submitted on-chain but expired before it was included in a block.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct Expired {
    #[serde(rename = "result")]
    pub result: String,
    #[serde(rename = "type")]
    pub r#type: Type,
}

impl Expired {
    /// The bid was submitted on-chain but expired before it was included in a block.
    pub fn new(result: String, r#type: Type) -> Expired {
        Expired { result, r#type }
    }
}
///
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub enum Type {
    #[serde(rename = "expired")]
    Expired,
}

impl Default for Type {
    fn default() -> Type {
        Self::Expired
    }
}
