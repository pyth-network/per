//! This test helper util allows the user to create different static and dynamic canned RPC responses
//! that are then returned by the rpc client using the RPC client custom sender mechanism.

use {
    anchor_lang::AccountSerialize,
    axum::async_trait,
    base64::{
        prelude::BASE64_STANDARD,
        Engine,
    },
    express_relay::state::ExpressRelayMetadata,
    serde_json::json,
    solana_client::{
        client_error::Result,
        rpc_request::RpcRequest,
    },
    solana_rpc_client::{
        nonblocking::rpc_client::RpcClient,
        rpc_client::RpcClientConfig,
        rpc_sender::{
            RpcSender,
            RpcTransportStats,
        },
    },
    solana_sdk::{
        account::Account,
        program_pack::Pack,
    },
    spl_token_2022::state::Account as TokenAccount,
    std::sync::Arc,
    tokio::sync::Mutex,
};

pub enum CannedRequestMatcher {
    AllByRequest(RpcRequest),
    MatchRequestAndParams(RpcRequest, serde_json::Value),
    MatchRequestAndParamsDynamically(
        RpcRequest,
        Box<dyn Fn(&serde_json::Value) -> bool + Send + Sync>,
    ),
}

impl CannedRequestMatcher {
    pub fn is_match(&self, request: &RpcRequest, params: &serde_json::Value) -> bool {
        match self {
            CannedRequestMatcher::AllByRequest(req) => req == request,
            CannedRequestMatcher::MatchRequestAndParams(req, p) => req == request && p == params,
            CannedRequestMatcher::MatchRequestAndParamsDynamically(req, f) => {
                req == request && f(params)
            }
        }
    }
}

pub type DynamicResultFn =
    Box<dyn Fn(&RpcRequest, serde_json::Value) -> Result<serde_json::Value> + Send + Sync>;

pub enum CannedResult {
    Static(serde_json::Value),
    DynamicByParams(DynamicResultFn),
}

impl CannedResult {
    pub fn get_result(
        &self,
        request: &RpcRequest,
        params: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        match self {
            CannedResult::Static(r) => Ok(r.clone()),
            CannedResult::DynamicByParams(f) => f(request, params.clone()),
        }
    }
}

pub struct CannedRequest {
    matcher:  CannedRequestMatcher,
    result:   CannedResult,
    hits:     usize,
    max_hits: Option<usize>,
}

impl CannedRequest {
    pub fn new(
        matcher: CannedRequestMatcher,
        result: CannedResult,
        max_hits: Option<usize>,
    ) -> Self {
        Self {
            matcher,
            result,
            hits: 0,
            max_hits,
        }
    }

    pub fn is_dead(&self) -> bool {
        self.max_hits.map(|max| self.hits >= max).unwrap_or(false)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TokenAccountWithLamports {
    pub token_account: TokenAccount,
    pub lamports:      u64,
}

pub struct RpcClientSvmTesterInner {
    canned_responses: Mutex<Vec<CannedRequest>>,
}

#[derive(Clone)]
pub struct RpcClientSvmTester(Arc<RpcClientSvmTesterInner>);

impl std::ops::Deref for RpcClientSvmTester {
    type Target = RpcClientSvmTesterInner;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl RpcClientSvmTester {
    pub fn new() -> Self {
        Self(Arc::new(RpcClientSvmTesterInner {
            canned_responses: Mutex::new(vec![]),
        }))
    }

    pub fn make_test_client(&self) -> RpcClient {
        RpcClient::new_sender(
            RpcClientTesterSender {
                tester: self.clone(),
            },
            RpcClientConfig::default(),
        )
    }

    /// Panics if not all the canned responses have been consumed during the test
    pub async fn check_all_uncanned(&self) {
        let canned_responses = self.canned_responses.lock().await;

        assert_eq!(canned_responses.len(), 0, "There are canned responses");
    }

    fn pack_account(account: Account) -> serde_json::Value {
        let b_data = BASE64_STANDARD.encode(&account.data);
        json!({
            "lamports": account.lamports,
            "data": [b_data, "base64"],
            "owner": account.owner.to_string(),
            "executable": account.executable,
            "rentEpoch": account.rent_epoch,
        })
    }

    /// Can the given account for the next get account request
    pub async fn can_next_account(&self, matcher: CannedRequestMatcher, account: Account) {
        let mut canned_responses = self.canned_responses.lock().await;

        canned_responses.push(CannedRequest::new(
            matcher,
            CannedResult::Static(json!({
                "context": { "slot": 1 },
                "value": Self::pack_account(account),
            })),
            Some(1),
        ));
    }

    /// Can the given accounts for the next get multiple accounts request
    pub async fn can_next_multi_accounts(
        &self,
        matcher: CannedRequestMatcher,
        accounts: Vec<Account>,
    ) {
        let mut canned_responses = self.canned_responses.lock().await;

        let accounts_json = accounts
            .into_iter()
            .map(Self::pack_account)
            .collect::<Vec<_>>();

        canned_responses.push(CannedRequest::new(
            matcher,
            CannedResult::Static(json!({
                "context": { "slot": 1 },
                "value": accounts_json,
            })),
            Some(1),
        ));
    }

    pub async fn can_next_account_as_metadata(&self, metadata: ExpressRelayMetadata) {
        let mut bytes = Vec::new();
        metadata.try_serialize(&mut bytes).expect("serialize acc");

        self.can_next_account(
            CannedRequestMatcher::AllByRequest(RpcRequest::GetAccountInfo),
            Account {
                lamports:   1,
                data:       bytes,
                owner:      Default::default(),
                executable: false,
                rent_epoch: 0,
            },
        )
        .await;
    }

    pub async fn can_next_multi_call_token_accounts(
        &self,
        accounts: Vec<TokenAccountWithLamports>,
    ) {
        let accounts_len = accounts.len();

        let sol_accounts = accounts
            .into_iter()
            .map(|token_account_with_lamports| {
                let mut data = vec![0u8; TokenAccount::LEN];
                token_account_with_lamports
                    .token_account
                    .pack_into_slice(&mut data);

                Account {
                    lamports: token_account_with_lamports.lamports,
                    data,
                    owner: Default::default(),
                    executable: false,
                    rent_epoch: 0,
                }
            })
            .collect::<Vec<_>>();
        self.can_next_multi_accounts(
            CannedRequestMatcher::MatchRequestAndParamsDynamically(
                RpcRequest::GetMultipleAccounts,
                Box::new(move |params| {
                    let pubkeys = &params.as_array().expect("array")[0];
                    let is_all_unique = pubkeys.as_array().expect("array").len() == accounts_len;

                    is_all_unique
                }),
            ),
            sol_accounts,
        )
        .await;
    }
}

#[derive(Clone)]
pub struct RpcClientTesterSender {
    tester: RpcClientSvmTester,
}

#[async_trait]
impl RpcSender for RpcClientTesterSender {
    async fn send(
        &self,
        request: RpcRequest,
        params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let mut canned_responses = self.tester.canned_responses.lock().await;
        let (idx, canned_match) = canned_responses
            .iter_mut()
            .enumerate()
            .find(|(_, canned_res)| canned_res.matcher.is_match(&request, &params))
            .expect("No canned response found for request");

        canned_match.hits += 1;
        let res = canned_match.result.get_result(&request, &params);

        if canned_match.is_dead() {
            canned_responses.remove(idx);
        }

        res
    }

    fn get_transport_stats(&self) -> RpcTransportStats {
        RpcTransportStats::default()
    }

    fn url(&self) -> String {
        "test".to_string()
    }
}
