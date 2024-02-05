use {
    crate::{
        api::{
            marketplace::OpportunityBid,
            SHOULD_EXIT,
        },
        auction::{
            get_simulation_call,
            MulticallReturn,
            MulticallStatus,
        },
        state::{
            ChainStore,
            SpoofInfo,
            Store,
            UnixTimestamp,
            VerifiedLiquidationOpportunity,
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
    std::{
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
};

abigen!(
    LiquidationAdapter,
    "../per_multicall/out/LiquidationAdapter.sol/LiquidationAdapter.json"
);
abigen!(ERC20, "../per_multicall/out/ERC20.sol/ERC20.json");
abigen!(WETH9, "../per_multicall/out/WETH9.sol/WETH9.json");

impl From<(Address, U256)> for TokenQty {
    fn from(token: (Address, U256)) -> Self {
        TokenQty {
            token:  token.0,
            amount: token.1,
        }
    }
}

/// Verify an opportunity by simulating the liquidation call and checking the result
/// Simulation is done by spoofing the balances and allowances of a random liquidator
/// Returns Ok(()) if the simulation is successful or if the tokens cannot be spoofed
pub async fn verify_opportunity(
    opportunity: VerifiedLiquidationOpportunity,
    chain_store: &ChainStore,
    per_operator: Address,
) -> Result<()> {
    let client = Arc::new(chain_store.provider.clone());
    let fake_wallet = LocalWallet::new(&mut rand::thread_rng());
    let mut fake_bid = OpportunityBid {
        opportunity_id: opportunity.id,
        liquidator:     fake_wallet.address(),
        valid_until:    U256::max_value(),
        permission_key: opportunity.permission_key.clone(),
        amount:         U256::zero(),
        signature:      Signature {
            v: 0,
            r: U256::zero(),
            s: U256::zero(),
        },
    };

    let digest = get_liquidation_digest(make_liquidator_params(
        opportunity.clone(),
        fake_bid.clone(),
    ))?;
    let signature = fake_wallet.sign_hash(digest)?;
    fake_bid.signature = signature;
    let params = make_liquidator_params(opportunity.clone(), fake_bid.clone());
    let per_calldata = LiquidationAdapter::new(chain_store.config.adapter_contract, client.clone())
        .call_liquidation(params)
        .calldata()
        .ok_or(anyhow!(
            "Failed to generate calldata for liquidation adapter"
        ))?;

    let call = get_simulation_call(
        per_operator,
        chain_store.provider.clone(),
        chain_store.config.clone(),
        opportunity.permission_key,
        vec![chain_store.config.adapter_contract],
        vec![per_calldata],
        vec![fake_bid.amount],
    )
    .tx;
    let mut state = spoof::State::default();
    let token_spoof_info = chain_store.token_spoof_info.read().await.clone();
    for (token, amount) in opportunity.repay_tokens.into_iter() {
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
            SpoofInfo::UnableToSpoof => return Ok(()), // unable to spoof, so we can't verify
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
                    chain_store.config.adapter_contract,
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
            let multicall_results: Vec<MulticallStatus> = result.multicall_statuses;
            if !multicall_results.iter().all(|x| x.external_success) {
                return Err(anyhow!("PER Simulation failed"));
            }
        }
        Err(e) => return Err(anyhow!(format!("Error decoding multicall result: {:?}", e))),
    }
    Ok(())
}

fn get_liquidation_digest(params: liquidation_adapter::LiquidationCallParams) -> Result<H256> {
    // this should reflect the verifyCalldata function in the LiquidationAdapter contract
    let data = Bytes::from(abi::encode(&[
        params.repay_tokens.into_token(),
        params.expected_receipt_tokens.into_token(),
        params.contract_address.into_token(),
        params.data.into_token(),
        params.value.into_token(),
        params.bid.into_token(),
    ]));
    // encode packed does not work correctly for U256 so we need to convert it to bytes first
    let nonce_bytes = Bytes::from(<[u8; 32]>::from(params.valid_until));
    let digest = H256(keccak256(abi::encode_packed(&[
        data.into_token(),
        nonce_bytes.into_token(),
    ])?));
    Ok(digest)
}

pub fn verify_signature(params: liquidation_adapter::LiquidationCallParams) -> Result<()> {
    let digest = get_liquidation_digest(params.clone())?;
    let signature = Signature::try_from(params.signature_liquidator.to_vec().as_slice())
        .map_err(|_x| anyhow!("Error reading signature"))?;
    let signer = signature
        .recover(RecoveryMessage::Hash(digest))
        .map_err(|x| anyhow!(x.to_string()))?;
    let is_matched = signer == params.liquidator;
    is_matched.then_some(()).ok_or_else(|| {
        anyhow!(format!(
            "Invalid signature. Expected signer: {}, Got: {}",
            params.liquidator, signer
        ))
    })
}

pub fn parse_revert_error(revert: &Bytes) -> Option<String> {
    let apdapter_decoded = liquidation_adapter::LiquidationAdapterErrors::decode_with_selector(
        revert,
    )
    .map(|decoded_error| {
        format!(
            "Liquidation Adapter Contract Revert Error: {:#?}",
            decoded_error
        )
    });
    let erc20_decoded = erc20::ERC20Errors::decode_with_selector(revert)
        .map(|decoded_error| format!("ERC20 Contract Revert Error: {:#?}", decoded_error));
    apdapter_decoded.or(erc20_decoded)
}

pub fn make_liquidator_params(
    opportunity: VerifiedLiquidationOpportunity,
    bid: OpportunityBid,
) -> liquidation_adapter::LiquidationCallParams {
    liquidation_adapter::LiquidationCallParams {
        repay_tokens:            opportunity
            .repay_tokens
            .into_iter()
            .map(TokenQty::from)
            .collect(),
        expected_receipt_tokens: opportunity
            .receipt_tokens
            .into_iter()
            .map(TokenQty::from)
            .collect(),
        liquidator:              bid.liquidator,
        contract_address:        opportunity.contract,
        data:                    opportunity.calldata,
        value:                   opportunity.value,
        valid_until:             bid.valid_until,
        bid:                     bid.amount,
        signature_liquidator:    bid.signature.to_vec().into(),
    }
}

pub async fn make_liquidator_calldata(
    opportunity: VerifiedLiquidationOpportunity,
    bid: OpportunityBid,
    provider: Provider<Http>,
    adapter_contract: Address,
) -> Result<Bytes> {
    let params = make_liquidator_params(opportunity, bid);
    verify_signature(params.clone())?;

    let client = Arc::new(provider);
    let calldata = LiquidationAdapter::new(adapter_contract, client.clone())
        .call_liquidation(params)
        .calldata()
        .ok_or(anyhow!(
            "Failed to generate calldata for liquidation adapter"
        ))?;

    Ok(calldata)
}

const MAX_STALE_OPPORTUNITY_SECS: i64 = 60;

/// Verify an opportunity is still valid by checking staleness and simulating the liquidation call and checking the result
/// Returns Ok(()) if the opportunity is still valid
///
/// # Arguments
///
/// * `opportunity`: opportunity to verify
/// * `store`: server store
async fn verify_with_store(
    opportunity: VerifiedLiquidationOpportunity,
    store: &Store,
) -> Result<()> {
    let current_time = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as UnixTimestamp;

    if current_time - opportunity.creation_time > MAX_STALE_OPPORTUNITY_SECS {
        return Err(anyhow!("Opportunity is stale"));
    }

    let chain_store = store
        .chains
        .get(&opportunity.chain_id)
        .ok_or(anyhow!("Chain not found: {}", opportunity.chain_id))?;
    let per_operator = store.per_operator.address();
    verify_opportunity(opportunity.clone(), chain_store, per_operator).await
}

/// Run an infinite loop to verify opportunities in the store and remove invalid ones
///
/// # Arguments
///
/// * `store`: server store
pub async fn run_verification_loop(store: Arc<Store>) {
    tracing::info!("Starting opportunity verifier...");
    while !SHOULD_EXIT.load(Ordering::Acquire) {
        let all_opportunities = store.liquidation_store.opportunities.read().await.clone();
        for (permission_key, opportunity) in all_opportunities.iter() {
            let should_remove = verify_with_store(opportunity.clone(), &store).await;
            match should_remove {
                Ok(_) => {}
                Err(e) => {
                    store
                        .liquidation_store
                        .opportunities
                        .write()
                        .await
                        .remove(permission_key);
                    tracing::info!("Removed Opportunity with failed verification: {}", e);
                }
            }
        }
        tokio::time::sleep(Duration::from_secs(5)).await; // this should be replaced by a subscription to the chain and trigger on new blocks
    }
    tracing::info!("Shutting down opportunity verifier...");
}
