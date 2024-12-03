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
pub enum OpportunityDeleteEvm {
    #[serde(rename = "v1_2")]
    V12(models::V12),
}

impl Default for OpportunityDeleteEvm {
    fn default() -> Self {
        Self::V12(Default::default())
    }
}
