use {
    crate::{
        api::{
            opportunity::OpportunityAdapterSignatureConfig,
            RestError,
        },
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
            UnixTimestampMicros,
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
            transaction::eip712::{
                EIP712Domain,
                Eip712,
                Eip712Error,
            },
            Address,
            Bytes,
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
        collections::HashMap,
        ops::Add,
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

pub async fn get_weth_address(
    adapter_contract: Address,
    provider: Provider<Http>,
) -> Result<Address> {
    let adapter = OpportunityAdapter::new(adapter_contract, Arc::new(provider));
    adapter
        .get_weth()
        .call()
        .await
        .map_err(|e| anyhow!("Error getting WETH address from adapter: {:?}", e))
}

/// Verify an opportunity by simulating the execution call and checking the result
/// Simulation is done by spoofing the balances and allowances of a random executor
/// Returns Ok(VerificationResult) if the simulation is successful or if the tokens cannot be spoofed
/// Returns Err if the simulation fails despite spoofing or if any other error occurs
pub async fn verify_opportunity(
    opportunity: OpportunityParamsV1,
    chain_store: &ChainStore,
    relayer: Address,
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

    let hashed_data =
        make_opportunity_execution_params(opportunity.clone(), fake_bid.clone(), chain_store)
            .encode_eip712()?;
    let signature = fake_wallet.sign_hash(hashed_data.into())?;
    fake_bid.signature = signature;
    let params =
        make_opportunity_execution_params(opportunity.clone(), fake_bid.clone(), chain_store)
            .params;
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
            Uuid::new_v4().to_bytes_le(),
            chain_store.config.opportunity_adapter_contract,
            adapter_calldata,
            fake_bid.amount,
        ))],
    )
    .tx;
    let mut state = spoof::State::default();
    let token_spoof_info = chain_store.token_spoof_info.read().await.clone();
    let mut required_tokens = opportunity.sell_tokens.clone();

    required_tokens.push(crate::state::TokenAmount {
        token:  chain_store.weth,
        amount: opportunity.target_call_value,
    });
    let mut tokens_map = HashMap::<Address, U256>::new();
    required_tokens.iter().for_each(|token_amount| {
        let amount = tokens_map.entry(token_amount.token).or_insert(U256::zero());
        *amount = amount.add(token_amount.amount);
    });

    for (token, amount) in tokens_map {
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

fn get_params_bytes(params: ExecutionParams) -> Bytes {
    Bytes::from(abi::encode(&[
        params.sell_tokens.into_token(),
        params.buy_tokens.into_token(),
        params.target_contract.into_token(),
        params.target_calldata.into_token(),
        params.target_call_value.into_token(),
        params.bid_amount.into_token(),
        params.valid_until.into_token(),
    ]))
}

impl Eip712 for OpportunityAdapterExecutionParams {
    type Error = Eip712Error;

    fn domain(
        &self,
    ) -> std::prelude::v1::Result<ethers::types::transaction::eip712::EIP712Domain, Self::Error>
    {
        let config = self.signature_config.clone();
        Ok(EIP712Domain {
            name:               config.domain_name.into(),
            version:            config.domain_version.into(),
            chain_id:           U256::from(config.chain_network_id).into(),
            verifying_contract: config.contract_address.into(),
            salt:               None,
        })
    }

    fn struct_hash(&self) -> std::prelude::v1::Result<[u8; 32], Self::Error> {
        let type_bytes = self.signature_config.opportunity_type.as_bytes();
        let type_hash = H256(keccak256(type_bytes));
        let data = Bytes::from(abi::encode(&[
            type_hash.into_token(),
            self.params.executor.into_token(),
            get_params_bytes(self.params.clone()).into_token(),
            self.params.valid_until.into_token(),
        ]));
        let digest = H256(keccak256(data));
        Ok(*digest.as_fixed_bytes())
    }

    fn type_hash() -> std::prelude::v1::Result<[u8; 32], Self::Error> {
        todo!()
    }
}

#[derive(ToSchema, Clone)]
pub struct OpportunityAdapterExecutionParams {
    params:           ExecutionParams,
    signature_config: OpportunityAdapterSignatureConfig,
}

fn verify_signature(execution_params: OpportunityAdapterExecutionParams) -> Result<()> {
    let structured_hash = execution_params.encode_eip712()?;
    let params = execution_params.params;
    let signature = Signature::try_from(params.signature.to_vec().as_slice())
        .map_err(|_x| anyhow!("Error reading signature"))?;
    let signer = signature
        .recover(structured_hash)
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
    chain_store: &ChainStore,
) -> OpportunityAdapterExecutionParams {
    OpportunityAdapterExecutionParams {
        params:           ExecutionParams {
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
        },
        signature_config: chain_store.into(),
    }
}

pub async fn make_adapter_calldata(
    opportunity: OpportunityParamsV1,
    bid: OpportunityBid,
    chain_store: &ChainStore,
) -> Result<Bytes> {
    let adapter_contract = chain_store.config.opportunity_adapter_contract;
    let execution_params = make_opportunity_execution_params(opportunity.clone(), bid, chain_store);
    verify_signature(execution_params.clone())?;

    let client = Arc::new(chain_store.provider.clone());
    let calldata = OpportunityAdapter::new(adapter_contract, client.clone())
        .execute_opportunity(execution_params.params)
        .calldata()
        .ok_or(anyhow!(
            "Failed to generate calldata for opportunity adapter"
        ))?;

    Ok(calldata)
}

const MAX_STALE_OPPORTUNITY_MICROS: i128 = 60_000_000;

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
    let relayer = store.relayer.address();
    match verify_opportunity(params.clone(), chain_store, relayer).await {
        Ok(VerificationResult::Success) => Ok(()),
        Ok(VerificationResult::UnableToSpoof) => {
            let current_time =
                SystemTime::now().duration_since(UNIX_EPOCH)?.as_micros() as UnixTimestampMicros;
            if current_time - opportunity.creation_time > MAX_STALE_OPPORTUNITY_MICROS {
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
                let all_opportunities = store.opportunity_store.opportunities.read().await.clone();
                for (_permission_key,opportunities) in all_opportunities.iter() {
                    // check each of the opportunities for this permission key for validity
                    for opportunity in opportunities.iter() {
                        match verify_with_store(opportunity.clone(), &store).await {
                            Ok(_) => {}
                            Err(e) => {
                                tracing::info!(
                                    "Removing Opportunity {} with failed verification: {}",
                                    opportunity.id,
                                    e
                                );
                                match store.remove_opportunity(opportunity).await {
                                    Ok(_) => {}
                                    Err(e) => {
                                        tracing::error!("Failed to remove opportunity: {}", e);
                                    }
                                }
                            }
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
        .read()
        .await
        .get(&opportunity_bid.permission_key)
        .ok_or(RestError::OpportunityNotFound)?
        .clone();

    let opportunity = opportunities
        .iter()
        .find(|o| o.id == opportunity_id)
        .ok_or(RestError::OpportunityNotFound)?;

    let OpportunityParams::V1(params) = &opportunity.params;

    let chain_store = store
        .chains
        .get(&params.chain_id)
        .ok_or(RestError::InvalidChainId)?;

    let adapter_calldata =
        make_adapter_calldata(params.clone(), opportunity_bid.clone(), chain_store)
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
        Ok(id) => Ok(id),
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

pub async fn get_signature_metadata(
    relayer: Address,
    provider: Provider<Http>,
    contract_address: Address,
) -> SignatureMetadata {
    let client = Arc::new(provider);
    let opportunity_adapter = OpportunityAdapter::new(contract_address, client);
    let call = opportunity_adapter.get_signature_metadata().from(relayer);

    match call.await {
        Ok(result) => result,
        Err(e) => {
            panic!(
                "Error calling opportunity adapter for signature config: {:?}",
                e
            );
        }
    }
}
