use {
    crate::{
        api::RestError,
        auction::{
            evaluate_simulation_results,
            get_simulation_call,
            handle_bid,
            Bid,
            MulticallData,
            MulticallReturn,
        },
        server::{
            EXIT_CHECK_INTERVAL,
            SHOULD_EXIT,
        },
        state::{
            BidAmount,
            ChainStore,
            Opportunity,
            OpportunityId,
            OpportunityParams,
            OpportunityParamsV1,
            SpoofInfo,
            Store,
            UnixTimestamp,
        },
        token_spoof,
    },
    anyhow::{
        anyhow,
        Result,
    },
    ethers::{
        abi::{
            AbiDecode,
            Tokenizable,
        },
        contract::{
            abigen,
            ContractRevert,
        },
        core::{
            abi,
            rand,
            utils::keccak256,
        },
        providers::{
            Http,
            Provider,
            RawCall,
        },
        signers::{
            LocalWallet,
            Signer,
        },
        types::{
            spoof,
            Address,
            Bytes,
            RecoveryMessage,
            Signature,
            H256,
            U256,
        },
    },
    serde::{
        Deserialize,
        Serialize,
    },
    std::{
        result,
        sync::{
            atomic::Ordering,
            Arc,
        },
        time::{
            Duration,
            SystemTime,
            UNIX_EPOCH,
        },
    },
    utoipa::ToSchema,
    uuid::Uuid,
};

abigen!(
    OpportunityAdapter,
    "../per_multicall/out/OpportunityAdapter.sol/OpportunityAdapter.json"
);
abigen!(ERC20, "../per_multicall/out/ERC20.sol/ERC20.json");
abigen!(WETH9, "../per_multicall/out/WETH9.sol/WETH9.json");


pub enum VerificationResult {
    Success,
    UnableToSpoof,
}

/// Verify an opportunity by simulating the execution call and checking the result
/// Simulation is done by spoofing the balances and allowances of a random executor
/// Returns Ok(VerificationResult) if the simulation is successful or if the tokens cannot be spoofed
/// Returns Err if the simulation fails despite spoofing or if any other error occurs
pub async fn verify_opportunity(
    opportunity: OpportunityParamsV1,
    chain_store: &ChainStore,
    relayer: LocalWallet,
) -> Result<VerificationResult> {
    let client = Arc::new(chain_store.provider.clone());
    let fake_wallet = LocalWallet::new(&mut rand::thread_rng());
    let mut fake_bid = OpportunityBid {
        executor:       fake_wallet.address(),
        valid_until:    U256::max_value(),
        permission_key: opportunity.permission_key.clone(),
        amount:         U256::zero(),
        signature:      Signature {
            v: 0,
            r: U256::zero(),
            s: U256::zero(),
        },
    };

    let digest = get_params_digest(make_opportunity_execution_params(
        opportunity.clone(),
        fake_bid.clone(),
    ))?;
    let signature = fake_wallet.sign_hash(digest)?;
    fake_bid.signature = signature;
    let params = make_opportunity_execution_params(opportunity.clone(), fake_bid.clone());
    let adapter_calldata = OpportunityAdapter::new(
        chain_store.config.opportunity_adapter_contract,
        client.clone(),
    )
    .execute_opportunity(params)
    .calldata()
    .ok_or(anyhow!(
        "Failed to generate calldata for opportunity adapter"
    ))?;

    let call = get_simulation_call(
        relayer,
        chain_store.provider.clone(),
        chain_store.config.clone(),
        opportunity.permission_key,
        vec![MulticallData::from((
            chain_store.config.opportunity_adapter_contract,
            adapter_calldata,
            fake_bid.amount,
        ))],
    )
    .await
    .map_err(|_err| anyhow!("Error getting simulation call"))?
    .tx;
    let mut state = spoof::State::default();
    let token_spoof_info = chain_store.token_spoof_info.read().await.clone();
    for crate::state::TokenAmount { token, amount } in opportunity.sell_tokens.into_iter() {
        let spoof_info = match token_spoof_info.get(&token) {
            Some(info) => info.clone(),
            None => {
                let result = token_spoof::find_spoof_info(token, client.clone())
                    .await
                    .unwrap_or_else(|e| {
                        tracing::error!("Error finding spoof info: {:?}", e);
                        SpoofInfo::UnableToSpoof
                    });

                chain_store
                    .token_spoof_info
                    .write()
                    .await
                    .insert(token, result.clone());
                result
            }
        };
        match spoof_info {
            SpoofInfo::UnableToSpoof => return Ok(VerificationResult::UnableToSpoof),
            SpoofInfo::Spoofed {
                balance_slot,
                allowance_slot,
            } => {
                let balance_storage_key =
                    token_spoof::calculate_balance_storage_key(fake_wallet.address(), balance_slot);
                let value: [u8; 32] = amount.into();
                state
                    .account(token)
                    .store(balance_storage_key, value.into());

                let allowance_storage_key = token_spoof::calculate_allowance_storage_key(
                    fake_wallet.address(),
                    chain_store.config.opportunity_adapter_contract,
                    allowance_slot,
                );
                let value: [u8; 32] = amount.into();
                state
                    .account(token)
                    .store(allowance_storage_key, value.into());
            }
        }
    }
    let result = client.call_raw(&call).state(&state).await?;

    match MulticallReturn::decode(&result) {
        Ok(result) => {
            evaluate_simulation_results(result.multicall_statuses)
                .map_err(|_| anyhow!("Express Relay Simulation failed"))?;
        }
        Err(e) => return Err(anyhow!(format!("Error decoding multicall result: {:?}", e))),
    }
    Ok(VerificationResult::Success)
}

fn get_params_digest(params: ExecutionParams) -> Result<H256> {
    // this should reflect the verifyCalldata function in the OpportunityAdapter contract
    let data = Bytes::from(abi::encode(&[
        params.sell_tokens.into_token(),
        params.buy_tokens.into_token(),
        params.target_contract.into_token(),
        params.target_calldata.into_token(),
        params.target_call_value.into_token(),
        params.bid_amount.into_token(),
        params.valid_until.into_token(),
    ]));
    let digest = H256(keccak256(data));
    Ok(digest)
}

pub fn verify_signature(params: ExecutionParams) -> Result<()> {
    let digest = get_params_digest(params.clone())?;
    let signature = Signature::try_from(params.signature.to_vec().as_slice())
        .map_err(|_x| anyhow!("Error reading signature"))?;
    let signer = signature
        .recover(RecoveryMessage::Hash(digest))
        .map_err(|x| anyhow!(x.to_string()))?;
    let is_matched = signer == params.executor;
    is_matched.then_some(()).ok_or_else(|| {
        anyhow!(format!(
            "Invalid signature. Expected signer: {}, Got: {}",
            params.executor, signer
        ))
    })
}

pub fn parse_revert_error(revert: &Bytes) -> Option<String> {
    let apdapter_decoded =
        OpportunityAdapterErrors::decode_with_selector(revert).map(|decoded_error| {
            format!(
                "Opportunity Adapter Contract Revert Error: {:#?}",
                decoded_error
            )
        });
    let erc20_decoded = erc20::ERC20Errors::decode_with_selector(revert)
        .map(|decoded_error| format!("ERC20 Contract Revert Error: {:#?}", decoded_error));
    apdapter_decoded.or(erc20_decoded)
}

impl From<crate::state::TokenAmount> for TokenAmount {
    fn from(token: crate::state::TokenAmount) -> Self {
        TokenAmount {
            token:  token.token,
            amount: token.amount,
        }
    }
}
pub fn make_opportunity_execution_params(
    opportunity: OpportunityParamsV1,
    bid: OpportunityBid,
) -> ExecutionParams {
    ExecutionParams {
        sell_tokens:       opportunity
            .sell_tokens
            .into_iter()
            .map(TokenAmount::from)
            .collect(),
        buy_tokens:        opportunity
            .buy_tokens
            .into_iter()
            .map(TokenAmount::from)
            .collect(),
        executor:          bid.executor,
        target_contract:   opportunity.target_contract,
        target_calldata:   opportunity.target_calldata,
        target_call_value: opportunity.target_call_value,
        valid_until:       bid.valid_until,
        bid_amount:        bid.amount,
        signature:         bid.signature.to_vec().into(),
    }
}

pub async fn make_adapter_calldata(
    opportunity: OpportunityParamsV1,
    bid: OpportunityBid,
    provider: Provider<Http>,
    adapter_contract: Address,
) -> Result<Bytes> {
    let params = make_opportunity_execution_params(opportunity, bid);
    verify_signature(params.clone())?;

    let client = Arc::new(provider);
    let calldata = OpportunityAdapter::new(adapter_contract, client.clone())
        .execute_opportunity(params)
        .calldata()
        .ok_or(anyhow!(
            "Failed to generate calldata for opportunity adapter"
        ))?;

    Ok(calldata)
}

const MAX_STALE_OPPORTUNITY_SECS: i64 = 60;

/// Verify an opportunity is still valid by checking staleness and simulating the execution call and checking the result
/// Returns Ok(()) if the opportunity is still valid
///
/// # Arguments
///
/// * `opportunity`: opportunity to verify
/// * `store`: server store
async fn verify_with_store(opportunity: Opportunity, store: &Store) -> Result<()> {
    let OpportunityParams::V1(params) = opportunity.params;
    let chain_store = store
        .chains
        .get(&params.chain_id)
        .ok_or(anyhow!("Chain not found: {}", params.chain_id))?;
    let relayer = store.relayer.clone();
    match verify_opportunity(params.clone(), chain_store, relayer).await {
        Ok(VerificationResult::Success) => Ok(()),
        Ok(VerificationResult::UnableToSpoof) => {
            let current_time =
                SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as UnixTimestamp;
            if current_time - opportunity.creation_time > MAX_STALE_OPPORTUNITY_SECS {
                Err(anyhow!("Opportunity is stale and unverifiable"))
            } else {
                Ok(())
            }
        }
        Err(e) => Err(e),
    }
}

/// Run an infinite loop to verify opportunities in the store and remove invalid ones
///
/// # Arguments
///
/// * `store`: server store
pub async fn run_verification_loop(store: Arc<Store>) -> Result<()> {
    tracing::info!("Starting opportunity verifier...");
    let mut exit_check_interval = tokio::time::interval(EXIT_CHECK_INTERVAL);

    // this should be replaced by a subscription to the chain and trigger on new blocks
    let mut submission_interval = tokio::time::interval(Duration::from_secs(5));
    while !SHOULD_EXIT.load(Ordering::Acquire) {
        tokio::select! {
            _ = submission_interval.tick() => {
                let all_opportunities = store.opportunity_store.opportunities.clone();
                for item in all_opportunities.iter() {
                    // check each of the opportunities for this permission key for validity
                    let mut opps_to_remove = vec![];
                    for opportunity in item.value().iter() {
                        match verify_with_store(opportunity.clone(), &store).await {
                            Ok(_) => {}
                            Err(e) => {
                                opps_to_remove.push(opportunity.id);
                                tracing::info!(
                                    "Removing Opportunity {} with failed verification: {}",
                                    opportunity.id,
                                    e
                                );
                            }
                        }
                    }
                    let permission_key = item.key();
                    let opportunities_map = &store.opportunity_store.opportunities;
                    if let Some(mut opportunities) = opportunities_map.get_mut(permission_key) {
                        opportunities.retain(|x| !opps_to_remove.contains(&x.id));
                        if opportunities.is_empty() {
                            drop(opportunities);
                            opportunities_map.remove(permission_key);
                        }
                    }
                }
            }
            _ = exit_check_interval.tick() => {
            }
        }
    }
    tracing::info!("Shutting down opportunity verifier...");
    Ok(())
}

#[derive(Serialize, Deserialize, ToSchema, Clone)]
pub struct OpportunityBid {
    /// The opportunity permission key
    #[schema(example = "0xdeadbeefcafe", value_type=String)]
    pub permission_key: Bytes,
    /// The bid amount in wei.
    #[schema(example = "1000000000000000000", value_type=String)]
    #[serde(with = "crate::serde::u256")]
    pub amount:         BidAmount,
    /// How long the bid will be valid for.
    #[schema(example = "1000000000000000000", value_type=String)]
    #[serde(with = "crate::serde::u256")]
    pub valid_until:    U256,
    /// Executor address
    #[schema(example = "0x5FbDB2315678afecb367f032d93F642f64180aa2", value_type=String)]
    pub executor:       abi::Address,
    #[schema(
        example = "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef12",
        value_type=String
    )]
    #[serde(with = "crate::serde::signature")]
    pub signature:      Signature,
}

pub async fn handle_opportunity_bid(
    store: Arc<Store>,
    opportunity_id: OpportunityId,
    opportunity_bid: &OpportunityBid,
) -> result::Result<Uuid, RestError> {
    let opportunities = store
        .opportunity_store
        .opportunities
        .get(&opportunity_bid.permission_key)
        .ok_or(RestError::OpportunityNotFound)?
        .clone();

    let opportunity = opportunities
        .iter()
        .find(|o| o.id == opportunity_id)
        .ok_or(RestError::OpportunityNotFound)?;

    // TODO: move this logic to searcher side
    if opportunity.bidders.contains(&opportunity_bid.executor) {
        return Err(RestError::BadParameters(
            "Executor already bid on this opportunity".to_string(),
        ));
    }

    let OpportunityParams::V1(params) = &opportunity.params;

    let chain_store = store
        .chains
        .get(&params.chain_id)
        .ok_or(RestError::InvalidChainId)?;

    let adapter_calldata = make_adapter_calldata(
        params.clone(),
        opportunity_bid.clone(),
        chain_store.provider.clone(),
        chain_store.config.opportunity_adapter_contract,
    )
    .await
    .map_err(|e| RestError::BadParameters(e.to_string()))?;
    match handle_bid(
        store.clone(),
        Bid {
            permission_key:  params.permission_key.clone(),
            chain_id:        params.chain_id.clone(),
            target_contract: chain_store.config.opportunity_adapter_contract,
            target_calldata: adapter_calldata,
            amount:          opportunity_bid.amount,
        },
    )
    .await
    {
        Ok(id) => {
            let opportunities = store
                .opportunity_store
                .opportunities
                .get_mut(&opportunity_bid.permission_key);
            if let Some(mut opportunities) = opportunities {
                let opportunity = opportunities
                    .iter_mut()
                    .find(|o| o.id == opportunity_id)
                    .ok_or(RestError::OpportunityNotFound)?;
                opportunity.bidders.insert(opportunity_bid.executor);
            }
            Ok(id)
        }
        Err(e) => match e {
            RestError::SimulationError { result, reason } => {
                let parsed = parse_revert_error(&result);
                match parsed {
                    Some(decoded) => Err(RestError::BadParameters(decoded)),
                    None => {
                        tracing::info!("Could not parse revert reason: {}", reason);
                        Err(RestError::SimulationError { result, reason })
                    }
                }
            }
            _ => Err(e),
        },
    }
}
