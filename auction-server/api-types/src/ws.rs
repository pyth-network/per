use {
    crate::{
        bid::{
            BidCancel,
            BidCreate,
            BidResult,
            BidStatusWithId,
        },
        opportunity::{
            Opportunity,
            OpportunityBidEvm,
            OpportunityDelete,
            OpportunityId,
        },
        ChainId,
        Routable,
        SvmChainUpdate,
    },
    http::Method,
    serde::{
        Deserialize,
        Serialize,
    },
    strum::AsRefStr,
    utoipa::ToSchema,
};


#[derive(Deserialize, Clone, ToSchema, Serialize)]
#[serde(tag = "method", content = "params")]
pub enum ClientMessage {
    #[serde(rename = "subscribe")]
    Subscribe {
        #[schema(value_type = Vec<String>)]
        chain_ids: Vec<ChainId>,
    },
    #[serde(rename = "unsubscribe")]
    Unsubscribe {
        #[schema(value_type = Vec<String>)]
        chain_ids: Vec<ChainId>,
    },
    #[serde(rename = "post_bid")]
    PostBid { bid: BidCreate },

    #[serde(rename = "post_opportunity_bid")]
    PostOpportunityBid {
        #[schema(value_type = String)]
        opportunity_id:  OpportunityId,
        opportunity_bid: OpportunityBidEvm,
    },

    #[serde(rename = "cancel_bid")]
    CancelBid { data: BidCancel },
}

#[derive(Deserialize, Clone, ToSchema, Serialize)]
pub struct ClientRequest {
    pub id:  String,
    #[serde(flatten)]
    pub msg: ClientMessage,
}

/// This enum is used to send an update to the client for any subscriptions made.
#[derive(Serialize, Clone, ToSchema, Deserialize, Debug)]
#[serde(tag = "type")]
#[allow(clippy::large_enum_variant)]
pub enum ServerUpdateResponse {
    #[serde(rename = "new_opportunity")]
    NewOpportunity { opportunity: Opportunity },
    #[serde(rename = "bid_status_update")]
    BidStatusUpdate { status: BidStatusWithId },
    #[serde(rename = "svm_chain_update")]
    SvmChainUpdate { update: SvmChainUpdate },
    #[serde(rename = "remove_opportunities")]
    RemoveOpportunities {
        opportunity_delete: OpportunityDelete,
    },
}

#[derive(Serialize, Clone, ToSchema, Deserialize, Debug)]
#[serde(untagged)]
pub enum APIResponse {
    BidResult(BidResult),
}
#[derive(Serialize, Clone, ToSchema, Deserialize, Debug)]
#[serde(tag = "status", content = "result")]
pub enum ServerResultMessage {
    #[serde(rename = "success")]
    Success(Option<APIResponse>),
    #[serde(rename = "error")]
    Err(String),
}

/// This enum is used to send the result for a specific client request with the same id.
/// Id is only None when the client message is invalid.
#[derive(Serialize, ToSchema, Deserialize, Clone, Debug)]
pub struct ServerResultResponse {
    pub id:     Option<String>,
    #[serde(flatten)]
    pub result: ServerResultMessage,
}

#[derive(AsRefStr, Clone)]
#[strum(prefix = "/")]
pub enum Route {
    #[strum(serialize = "ws")]
    Ws,
}

impl Routable for Route {
    fn properties(&self) -> crate::RouteProperties {
        let full_path = format!("{}{}", crate::Route::V1.as_ref(), self.as_ref())
            .trim_end_matches('/')
            .to_string();
        match self {
            Route::Ws => crate::RouteProperties {
                access_level: crate::AccessLevel::Public,
                method: Method::GET,
                full_path,
            },
        }
    }
}
