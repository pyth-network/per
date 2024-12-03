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
pub struct Pending {
    #[serde(rename = "type")]
    pub r#type: Type,
}

impl Pending {
    pub fn new(r#type: Type) -> Pending {
        Pending { r#type }
    }
}
///
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub enum Type {
    #[serde(rename = "pending")]
    Pending,
}

impl Default for Type {
    fn default() -> Type {
        Self::Pending
    }
}
