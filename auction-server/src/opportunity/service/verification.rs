use {
    super::{
        get_spoof_info::GetSpoofInfoInput,
        make_adapter_calldata::MakeAdapterCalldataInput,
        make_opportunity_execution_params::MakeOpportunityExecutionParamsInput,
        ChainType,
        ChainTypeEvm,
        ChainTypeSvm,
        Service,
    },
    crate::{
        api::RestError,
        kernel::contracts::{
            ExecutionParams,
            MulticallData,
            MulticallReturn,
        },
        opportunity::{
            entities,
            repository::InMemoryStore,
            token_spoof,
        },
    },
    ethers::{
        abi::AbiDecode,
        providers::RawCall,
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
            Signature,
            U256,
        },
        utils::get_create2_address_from_hash,
    },
    express_relay_api_types::opportunity::OpportunityBidEvm,
    rand::Rng,
    std::{
        collections::HashMap,
        future::Future,
        ops::Add,
        sync::Arc,
    },
    uuid::Uuid,
};

pub struct VerifyOpportunityInput<T: entities::OpportunityCreate> {
    pub opportunity: T,
}

pub trait Verification<T: ChainType> {
    fn verify_opportunity(
        &self,
        input: VerifyOpportunityInput<<<T::InMemoryStore as InMemoryStore>::Opportunity as entities::Opportunity>::OpportunityCreate>,
    ) -> impl Future<Output = Result<entities::OpportunityVerificationResult, RestError>>;
}

fn generate_random_u256() -> U256 {
    let mut rng = rand::thread_rng();
    U256::from(rng.gen::<[u8; 32]>())
}

fn get_typed_data(
    params: ExecutionParams,
    spender: Address,
    domain: eip712::EIP712Domain,
) -> eip712::TypedData {
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
        "spender": spender,
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
        domain,
        types: serde_json::from_value(data_type)
            .expect("Failed to parse data type for eip712 typed data"),
        primary_type: "PermitBatchWitnessTransferFrom".into(),
        message: serde_json::from_value(data).expect("Failed to parse data for eip712 typed data"),
    }
}

impl Verification<ChainTypeEvm> for Service<ChainTypeEvm> {
    async fn verify_opportunity(
        &self,
        input: VerifyOpportunityInput<entities::OpportunityCreateEvm>,
    ) -> Result<entities::OpportunityVerificationResult, RestError> {
        let config = self.get_config(&input.opportunity.core_fields.chain_id)?;
        let auction_service = config.get_auction_service().await;
        let client = Arc::new(config.provider.clone());
        let fake_wallet = LocalWallet::new(&mut rand::thread_rng());

        let mut fake_bid = OpportunityBidEvm {
            executor:       fake_wallet.address(),
            deadline:       U256::max_value(),
            nonce:          generate_random_u256(),
            permission_key: input.opportunity.core_fields.permission_key.clone(),
            amount:         U256::zero(),
            signature:      Signature {
                v: 0,
                r: U256::zero(),
                s: U256::zero(),
            },
        };

        let mut salt = [0u8; 32];
        salt[12..32].copy_from_slice(fake_bid.executor.as_bytes());
        let executor_adapter_address = get_create2_address_from_hash(
            config.adapter_factory_contract,
            salt,
            config.adapter_bytecode_hash,
        );

        let eip_712_domain = EIP712Domain {
            name:               Some("Permit2".to_string()),
            version:            None,
            chain_id:           Some(config.chain_id_num.into()),
            verifying_contract: Some(config.permit2),
            salt:               None,
        };

        let params =
            self.make_opportunity_execution_params(MakeOpportunityExecutionParamsInput {
                opportunity:     input.opportunity.clone(),
                opportunity_bid: fake_bid.clone(),
            })?;

        let typed_data: eip712::TypedData =
            get_typed_data(params.clone(), executor_adapter_address, eip_712_domain);
        let hashed_data = typed_data.encode_eip712().map_err(|e| {
            tracing::error!("Error encoding eip712 data: {:?}", e);
            RestError::TemporarilyUnavailable
        })?;
        fake_bid.signature = fake_wallet.sign_hash(hashed_data.into()).map_err(|e| {
            tracing::error!("Error signing eip712 data: {:?}", e);
            RestError::TemporarilyUnavailable
        })?;

        let adapter_calldata = self.make_adapter_calldata(MakeAdapterCalldataInput {
            opportunity:     input.opportunity.clone(),
            opportunity_bid: fake_bid.clone(),
        })?;

        let call = auction_service
            .get_simulation_call(
                input.opportunity.core_fields.permission_key.clone(),
                vec![MulticallData::from((
                    Uuid::new_v4().to_bytes_le(),
                    config.adapter_factory_contract,
                    adapter_calldata,
                    fake_bid.amount,
                    U256::max_value(),
                    false,
                ))],
            )
            .tx;
        let mut state = spoof::State::default();
        let required_tokens = params.permit.permitted.clone();
        let mut tokens_map = HashMap::<Address, U256>::new();
        required_tokens.iter().for_each(|token_amount| {
            let amount = tokens_map.entry(token_amount.token).or_insert(U256::zero());
            *amount = amount.add(token_amount.amount);
        });

        for (token, amount) in tokens_map {
            let spoof_info = self
                .get_spoof_info(GetSpoofInfoInput {
                    chain_id: input.opportunity.core_fields.chain_id.clone(),
                    token,
                })
                .await?;
            match spoof_info.state {
                entities::SpoofState::UnableToSpoof => {
                    return Ok(entities::OpportunityVerificationResult::UnableToSpoof)
                }
                entities::SpoofState::Spoofed {
                    balance_slot,
                    allowance_slot,
                } => {
                    let balance_storage_key = token_spoof::calculate_balance_storage_key(
                        fake_wallet.address(),
                        balance_slot,
                    );
                    let value: [u8; 32] = amount.into();
                    state
                        .account(token)
                        .store(balance_storage_key, value.into());

                    let allowance_storage_key = token_spoof::calculate_allowance_storage_key(
                        fake_wallet.address(),
                        config.permit2,
                        allowance_slot,
                    );
                    let value: [u8; 32] = amount.into();
                    state
                        .account(token)
                        .store(allowance_storage_key, value.into());
                }
            }
        }
        match client.call_raw(&call).state(&state).await {
            Ok(result) => match MulticallReturn::decode(&result) {
                Ok(result) => {
                    if result.multicall_statuses[0].external_success {
                        Ok(entities::OpportunityVerificationResult::Success)
                    } else {
                        tracing::info!(
                            "Opportunity simulation failed: {:?}",
                            result.multicall_statuses
                        );
                        Err(RestError::InvalidOpportunity(format!(
                            "Express Relay Simulation failed: {:?}",
                            result.multicall_statuses
                        )))
                    }
                }
                Err(e) => Err(RestError::InvalidOpportunity(format!(
                    "Error decoding multicall result: {:?} - result: {:?}",
                    e, result
                ))),
            },
            Err(e) => {
                tracing::error!("Error calling relay contract: {:?}", e);
                Err(RestError::TemporarilyUnavailable)
            }
        }
    }
}

impl Verification<ChainTypeSvm> for Service<ChainTypeSvm> {
    async fn verify_opportunity(
        &self,
        input: VerifyOpportunityInput<entities::OpportunityCreateSvm>,
    ) -> Result<entities::OpportunityVerificationResult, RestError> {
        self.get_config(&input.opportunity.core_fields.chain_id)?;

        // To make sure it'll be expired after a minute
        // TODO - change this to a more realistic value
        Ok(entities::OpportunityVerificationResult::UnableToSpoof)
    }
}
