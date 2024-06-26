use {
    crate::{
        api::{
            Auth,
            RestError,
        },
        auction::{
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
        traced_client::TracedClient,
    },
    anyhow::{
        anyhow,
        Result,
    },
    ethers::{
        abi::AbiDecode,
        contract::{
            abigen,
            ContractRevert,
        },
        core::{
            abi,
            rand,
        },
        providers::{
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
                self,
                EIP712Domain,
                Eip712,
            },
            Address,
            Bytes,
            Signature,
            U256,
        },
        utils::get_create2_address_from_hash,
    },
    rand::Rng,
    serde::{
        Deserialize,
        Serialize,
    },
    sqlx::types::time::OffsetDateTime,
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
    "../contracts/out/OpportunityAdapter.sol/OpportunityAdapter.json";
    AdapterFactory,
    "../contracts/out/OpportunityAdapterFactory.sol/OpportunityAdapterFactory.json"
);
abigen!(ERC20, "../contracts/out/ERC20.sol/ERC20.json");
abigen!(WETH9, "../contracts/out/WETH9.sol/WETH9.json");

pub enum VerificationResult {
    Success,
    UnableToSpoof,
}

pub async fn get_weth_address(
    adapter_contract: Address,
    provider: Provider<TracedClient>,
) -> Result<Address> {
    let adapter = AdapterFactory::new(adapter_contract, Arc::new(provider));
    adapter
        .get_weth()
        .call()
        .await
        .map_err(|e| anyhow!("Error getting WETH address from adapter: {:?}", e))
}

pub async fn get_adapter_bytecode_hash(
    adapter_contract: Address,
    provider: Provider<TracedClient>,
) -> Result<[u8; 32]> {
    let adapter = AdapterFactory::new(adapter_contract, Arc::new(provider));
    adapter
        .get_opportunity_adapter_creation_code_hash()
        .call()
        .await
        .map_err(|e| anyhow!("Error getting adapter code hash from adapter: {:?}", e))
}

pub async fn get_permit2_address(
    adapter_contract: Address,
    provider: Provider<TracedClient>,
) -> Result<Address> {
    let adapter = AdapterFactory::new(adapter_contract, Arc::new(provider));
    adapter
        .get_permit_2()
        .call()
        .await
        .map_err(|e| anyhow!("Error getting permit2 address from adapter: {:?}", e))
}

fn generate_random_u256() -> U256 {
    let mut rng = rand::thread_rng();
    U256::from(rng.gen::<[u8; 32]>())
}

/// Verify an opportunity by simulating the execution call and checking the result
/// Simulation is done by spoofing the balances and allowances of a random executor
/// Returns Ok(VerificationResult) if the simulation is successful or if the tokens cannot be spoofed
/// Returns Err if the simulation fails despite spoofing or if any other error occurs
#[tracing::instrument(skip_all)]
pub async fn verify_opportunity(
    opportunity: OpportunityParamsV1,
    chain_store: &ChainStore,
    relayer: Address,
) -> Result<VerificationResult> {
    let client = Arc::new(chain_store.provider.clone());
    let fake_wallet = LocalWallet::new(&mut rand::thread_rng());

    let fake_bid = OpportunityBid {
        executor:       fake_wallet.address(),
        deadline:       U256::max_value(),
        nonce:          generate_random_u256(),
        permission_key: opportunity.permission_key.clone(),
        amount:         U256::zero(),
        signature:      Signature {
            v: 0,
            r: U256::zero(),
            s: U256::zero(),
        },
    };

    let params_with_signature =
        make_opportunity_execution_params(opportunity.clone(), fake_bid.clone(), chain_store);
    let typed_data: eip712::TypedData = params_with_signature.clone().into();
    let hashed_data = typed_data.encode_eip712()?;
    let signature = fake_wallet.sign_hash(hashed_data.into())?;

    let adapter_calldata =
        AdapterFactory::new(chain_store.config.adapter_factory_contract, client.clone())
            .execute_opportunity(
                params_with_signature.params.clone(),
                signature.to_vec().into(),
            )
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
            chain_store.config.adapter_factory_contract,
            adapter_calldata,
            fake_bid.amount,
            U256::max_value(),
            false,
        ))],
    )
    .tx;
    let mut state = spoof::State::default();
    let token_spoof_info = chain_store.token_spoof_info.read().await.clone();
    let required_tokens = params_with_signature.params.permit.permitted.clone();
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
                    chain_store.permit2,
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
            if !result.multicall_statuses[0].external_success {
                tracing::info!(
                    "Opportunity simulation failed: {:?}",
                    result.multicall_statuses[0]
                );
                return Err(anyhow!(
                    "Express Relay Simulation failed: {:?}",
                    result.multicall_statuses[0].external_result
                ));
            }
        }
        Err(e) => return Err(anyhow!(format!("Error decoding multicall result: {:?}", e))),
    }
    Ok(VerificationResult::Success)
}
impl From<ExecutionParamsWithSignature> for eip712::TypedData {
    fn from(val: ExecutionParamsWithSignature) -> Self {
        let params = val.params;
        let data_type = serde_json::json!({
            "PermitBatchWitnessTransferFrom": [
                {"name": "permitted", "type": "TokenPermissions[]"},
                {"name": "spender", "type": "address"},
                {"name": "nonce", "type": "uint256"},
                {"name": "deadline", "type": "uint256"},
                {"name": "witness", "type": "OpportunityWitness"},
            ],
            "OpportunityWitness": [
                {"name": "buyTokens", "type": "TokenAmount[]"},
                {"name": "executor", "type": "address"},
                {"name": "targetContract", "type": "address"},
                {"name": "targetCalldata", "type": "bytes"},
                {"name": "targetCallValue", "type": "uint256"},
                {"name": "bidAmount", "type": "uint256"},
            ],
            "TokenAmount": [
                {"name": "token", "type": "address"},
                {"name": "amount", "type": "uint256"},
            ],
            "TokenPermissions": [
                {"name": "token", "type": "address"},
                {"name": "amount", "type": "uint256"},
            ],
        });
        let data = serde_json::json!({
            "permitted": params.permit.permitted.into_iter().map(|x| serde_json::json!({
                "token": x.token,
                "amount": x.amount,
            })).collect::<Vec<_>>(),
            "spender": val.spender,
            "nonce": params.permit.nonce,
            "deadline": params.permit.deadline,
            "witness": serde_json::json!({
                "buyTokens": params.witness.buy_tokens.into_iter().map(|x| serde_json::json!({
                    "token": x.token,
                    "amount": x.amount,
                })).collect::<Vec<_>>(),
                "executor": params.witness.executor,
                "targetContract": params.witness.target_contract,
                "targetCalldata": params.witness.target_calldata,
                "targetCallValue": params.witness.target_call_value,
                "bidAmount": params.witness.bid_amount,
            }),
        });
        eip712::TypedData {
            domain:       val.eip_712_domain,
            types:        serde_json::from_value(data_type)
                .expect("Failed to parse data type for eip712 typed data"),
            primary_type: "PermitBatchWitnessTransferFrom".into(),
            message:      serde_json::from_value(data)
                .expect("Failed to parse data for eip712 typed data"),
        }
    }
}

#[derive(ToSchema, Clone)]
pub struct ExecutionParamsWithSignature {
    params:         ExecutionParams,
    eip_712_domain: EIP712Domain,
    spender:        Address, // Equal to the opportunity adapter contract
    signature:      Bytes,
}

fn verify_signature(execution_params: ExecutionParamsWithSignature) -> Result<()> {
    // TODO Maybe use ECDSA to recover the signer? https://docs.rs/k256/latest/k256/ecdsa/index.html
    let typed_data: eip712::TypedData = execution_params.clone().into();
    let structured_hash = typed_data.encode_eip712()?;
    let params = execution_params.params;
    let signature = Signature::try_from(execution_params.signature.to_vec().as_slice())
        .map_err(|_x| anyhow!("Error reading signature"))?;
    let signer = signature
        .recover(structured_hash)
        .map_err(|x| anyhow!(x.to_string()))?;
    let is_matched = signer == params.witness.executor;
    is_matched.then_some(()).ok_or_else(|| {
        anyhow!(format!(
            "Invalid signature. Expected signer: {}, Got: {}",
            params.witness.executor, signer
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

fn make_permitted_tokens(
    opportunity: OpportunityParamsV1,
    bid: OpportunityBid,
    chain_store: &ChainStore,
) -> Vec<TokenPermissions> {
    let mut permitted_tokens: Vec<TokenPermissions> = opportunity
        .sell_tokens
        .clone()
        .into_iter()
        .map(|token| TokenPermissions {
            token:  token.token,
            amount: token.amount,
        })
        .collect();

    let extra_weth_amount = bid.amount + opportunity.target_call_value;
    if let Some(weth_position) = permitted_tokens
        .iter()
        .position(|x| x.token == chain_store.weth)
    {
        permitted_tokens[weth_position] = TokenPermissions {
            amount: permitted_tokens[weth_position].amount + extra_weth_amount,
            ..permitted_tokens[weth_position]
        }
    } else if extra_weth_amount > U256::zero() {
        permitted_tokens.push(TokenPermissions {
            token:  chain_store.weth,
            amount: extra_weth_amount,
        });
    }
    permitted_tokens
}

pub fn make_opportunity_execution_params(
    opportunity: OpportunityParamsV1,
    bid: OpportunityBid,
    chain_store: &ChainStore,
) -> ExecutionParamsWithSignature {
    let mut salt = [0u8; 32];
    salt[12..32].copy_from_slice(bid.executor.as_bytes());
    let executor_adapter_address = get_create2_address_from_hash(
        chain_store.config.adapter_factory_contract,
        salt,
        chain_store.adapter_bytecode_hash,
    );
    let eip_712_domain = EIP712Domain {
        name:               Some("Permit2".to_string()),
        version:            None,
        chain_id:           Some(chain_store.chain_id_num.into()),
        verifying_contract: Some(chain_store.permit2),
        salt:               None,
    };
    ExecutionParamsWithSignature {
        params: ExecutionParams {
            permit:  PermitBatchTransferFrom {
                permitted: make_permitted_tokens(opportunity.clone(), bid.clone(), chain_store),
                nonce:     bid.nonce,
                deadline:  bid.deadline,
            },
            witness: ExecutionWitness {
                buy_tokens:        opportunity
                    .buy_tokens
                    .into_iter()
                    .map(TokenAmount::from)
                    .collect(),
                executor:          bid.executor,
                target_contract:   opportunity.target_contract,
                target_calldata:   opportunity.target_calldata,
                target_call_value: opportunity.target_call_value,
                bid_amount:        bid.amount,
            },
        },
        signature: bid.signature.to_vec().into(),
        eip_712_domain,
        spender: executor_adapter_address,
    }
}

pub async fn make_adapter_calldata(
    opportunity: OpportunityParamsV1,
    bid: OpportunityBid,
    chain_store: &ChainStore,
) -> Result<Bytes> {
    let adapter_contract = chain_store.config.adapter_factory_contract;
    let execution_params = make_opportunity_execution_params(opportunity.clone(), bid, chain_store);
    // TODO do we really need it here?
    verify_signature(execution_params.clone())?;

    let client = Arc::new(chain_store.provider.clone());
    let calldata = OpportunityAdapter::new(adapter_contract, client.clone())
        .execute_opportunity(
            execution_params.params,
            execution_params.signature.to_vec().into(),
        )
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
    /// The latest unix timestamp in seconds until which the bid is valid
    #[schema(example = "1000000000000000000", value_type=String)]
    #[serde(with = "crate::serde::u256")]
    pub deadline:       U256,
    /// The nonce of the bid permit signature
    #[schema(example = "123", value_type=String)]
    #[serde(with = "crate::serde::u256")]
    pub nonce:          U256,
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
    initiation_time: OffsetDateTime,
    auth: Auth,
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
            target_contract: chain_store.config.adapter_factory_contract,
            target_calldata: adapter_calldata,
            amount:          opportunity_bid.amount,
        },
        initiation_time,
        auth,
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
