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
#[serde(tag = "version")]
pub enum OpportunityParamsSvm {
    #[serde(rename = "v1_5")]
    V15(models::V15),
}

impl Default for OpportunityParamsSvm {
    fn default() -> Self {
        Self::V15(Default::default())
    }
}

///
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub enum Program {
    #[serde(rename = "phantom")]
    Phantom,
}

impl Default for Program {
    fn default() -> Program {
        Self::Phantom
    }
}
