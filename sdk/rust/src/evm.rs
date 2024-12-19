use {
    crate::ClientError,
    ethers::{
        abi::AbiEncode,
        contract::abigen,
        signers::{
            LocalWallet,
            Signer,
        },
        types::{
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
        utils::{
            get_create2_address_from_hash,
            hex,
        },
    },
    express_relay_api_types::opportunity::{
        OpportunityCreateV1Evm,
        OpportunityEvm,
        OpportunityParamsEvm,
        OpportunityParamsV1Evm,
    },
    std::collections::HashMap,
};

abigen!(
    OpportunityAdapter,
    "./abi/OpportunityAdapter.sol/OpportunityAdapter.json";
    AdapterFactory,
    "./abi/OpportunityAdapterFactory.sol/OpportunityAdapterFactory.json"
);
abigen!(ERC20, "./abi/ERC20.sol/ERC20.json");
abigen!(WETH9, "./abi/WETH9.sol/WETH9.json");

abigen!(ExpressRelay, "./abi/ExpressRelay.sol/ExpressRelay.json");

/// Retrieves opportunity parameters from an `OpportunityEvm` object.
///
/// # Arguments
///
/// * `opportunity` - The EVM opportunity structure.
///
/// # Returns
///
/// * `OpportunityCreateV1Evm` - The extracted opportunity parameters.
pub fn get_params(opportunity: OpportunityEvm) -> OpportunityCreateV1Evm {
    let OpportunityParamsEvm::V1(OpportunityParamsV1Evm(params)) = opportunity.params;
    params
}

#[derive(Debug, Clone)]
pub struct BidParamsEvm {
    pub amount:   ethers::types::U256,
    pub deadline: ethers::types::U256,
    pub nonce:    ethers::types::U256,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub weth:                     Address,
    pub adapter_factory_contract: Address,
    pub permit2:                  Address,
    pub adapter_bytecode_hash:    [u8; 32],
    pub chain_id_num:             u64,
}

pub struct Evm {
    config: HashMap<String, Config>,
}

impl Evm {
    /// Creates a new EVM configuration object.
    ///
    /// # Arguments
    ///
    /// * `config` - An optional configuration map.
    ///
    /// # Returns
    ///
    /// * `Self` - The EVM configuration object. If no configuration is provided, default configurations are used.
    pub fn new(config: Option<HashMap<String, Config>>) -> Self {
        match config {
            Some(config) => Self { config },
            None => {
                let mode_config = Config {
                    weth:                     "0x74A4A85C611679B73F402B36c0F84A7D2CcdFDa3"
                        .parse()
                        .expect("Invalid Ethereum address"),
                    permit2:                  "0x000000000022D473030F116dDEE9F6B43aC78BA3"
                        .parse()
                        .expect("Invalid Ethereum address"),
                    adapter_factory_contract: "0x59F78DE21a0b05d96Ae00c547BA951a3B905602f"
                        .parse()
                        .expect("Invalid Ethereum address"),
                    adapter_bytecode_hash:    hex::decode(
                        "0xd53b8e32ab2ecba07c3e3a17c3c5e492c62e2f7051b89e5154f52e6bfeb0e38f",
                    )
                    .expect("Invalid bytecode hash")
                    .try_into()
                    .expect("Invalid bytecode hash length"),
                    chain_id_num:             34443,
                };
                let op_sepolia_config = Config {
                    weth:                     "0x4200000000000000000000000000000000000006"
                        .parse()
                        .expect("Invalid Ethereum address"),
                    permit2:                  "0x000000000022D473030F116dDEE9F6B43aC78BA3"
                        .parse()
                        .expect("Invalid Ethereum address"),
                    adapter_factory_contract: "0xfA119693864b2F185742A409c66f04865c787754"
                        .parse()
                        .expect("Invalid Ethereum address"),
                    adapter_bytecode_hash:    hex::decode(
                        "0x3d71516d94b96a8fdca4e3a5825a6b41c9268a8e94610367e69a8462cc543533",
                    )
                    .expect("Invalid bytecode hash")
                    .try_into()
                    .expect("Invalid bytecode hash length"),
                    chain_id_num:             11155420,
                };
                let mut config = HashMap::new();
                config.insert("mode".to_string(), mode_config);
                config.insert("op_sepolia".to_string(), op_sepolia_config);
                Self { config }
            }
        }
    }

    /// Retrieves the EVM configuration for a specific chain.
    ///
    /// # Arguments
    ///
    /// * `chain_id` - A string slice representing the blockchain chain ID.
    ///
    /// # Returns
    ///
    /// * `Result<Config, ClientError>` - A result containing the configuration or an error if the chain is unsupported.
    ///
    /// # Errors
    ///
    /// Returns `ClientError::ChainNotSupported` if the chain ID is unsupported.
    pub fn get_config(&self, chain_id: &str) -> Result<Config, ClientError> {
        self.config
            .get(chain_id)
            .cloned()
            .ok_or(ClientError::ChainNotSupported)
    }

    /// Constructs the Permit2 compatible permitted tokens list for a given opportunity and bid parameters.
    ///
    /// # Arguments
    ///
    /// * `opportunity` - The EVM opportunity structure.
    /// * `bid_params` - Bid parameters.
    ///
    /// # Returns
    ///
    /// * `Result<Vec<TokenPermissions>, ClientError>` - A list of token permissions or an error.
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration for the chain cannot be retrieved.
    pub fn make_permitted_tokens(
        &self,
        opportunity: OpportunityEvm,
        bid_params: BidParamsEvm,
    ) -> Result<Vec<TokenPermissions>, ClientError> {
        let config = self.get_config(opportunity.get_chain_id())?;
        let params = get_params(opportunity);
        let mut permitted_tokens: Vec<TokenPermissions> = params
            .sell_tokens
            .clone()
            .into_iter()
            .map(|token| TokenPermissions {
                token:  token.token,
                amount: token.amount,
            })
            .collect();

        let extra_weth_amount = bid_params.amount + params.target_call_value;
        if let Some(weth_position) = permitted_tokens.iter().position(|x| x.token == config.weth) {
            permitted_tokens[weth_position] = TokenPermissions {
                amount: permitted_tokens[weth_position].amount + extra_weth_amount,
                ..permitted_tokens[weth_position]
            }
        } else if extra_weth_amount > U256::zero() {
            permitted_tokens.push(TokenPermissions {
                token:  config.weth,
                amount: extra_weth_amount,
            });
        }
        Ok(permitted_tokens)
    }

    /// Creates execution parameters required for executing an opportunity through the ER contract.
    ///
    /// # Arguments
    ///
    /// * `opportunity` - The EVM opportunity structure.
    /// * `bid_params` - Bid parameters.
    /// * `executor` - The address of the executor.
    ///
    /// # Returns
    ///
    /// * `Result<ExecutionParams, ClientError>` - Execution parameters including permit and witness details.
    ///
    /// # Errors
    ///
    /// Returns an error if Permit2 compatible permitted tokens cannot be constructed.
    pub fn make_opportunity_execution_params(
        &self,
        opportunity: OpportunityEvm,
        bid_params: BidParamsEvm,
        executor: Address,
    ) -> Result<ExecutionParams, ClientError> {
        let params = get_params(opportunity.clone());
        Ok(ExecutionParams {
            permit:  PermitBatchTransferFrom {
                permitted: self.make_permitted_tokens(opportunity, bid_params.clone())?,
                nonce:     bid_params.nonce,
                deadline:  bid_params.deadline,
            },
            witness: ExecutionWitness {
                buy_tokens: params
                    .buy_tokens
                    .clone()
                    .into_iter()
                    .map(|token| TokenAmount {
                        token:  token.token,
                        amount: token.amount,
                    })
                    .collect(),
                executor,
                target_contract: params.target_contract,
                target_calldata: params.target_calldata,
                target_call_value: params.target_call_value,
                bid_amount: bid_params.amount,
            },
        })
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
            message: serde_json::from_value(data)
                .expect("Failed to parse data for eip712 typed data"),
        }
    }

    fn get_signature(
        &self,
        opportunity: OpportunityEvm,
        bid_params: BidParamsEvm,
        wallet: LocalWallet,
    ) -> Result<Signature, ClientError> {
        let config = self.get_config(opportunity.get_chain_id())?;
        let mut salt = [0u8; 32];
        salt[12..32].copy_from_slice(wallet.address().as_bytes());
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

        let params = self.make_opportunity_execution_params(
            opportunity.clone(),
            bid_params.clone(),
            wallet.address(),
        )?;

        let typed_data: eip712::TypedData =
            Evm::get_typed_data(params.clone(), executor_adapter_address, eip_712_domain);
        let hashed_data = typed_data.encode_eip712().map_err(|e| {
            ClientError::NewBidError(format!("Failed to encode eip712 data: {:?}", e))
        })?;

        wallet
            .sign_hash(hashed_data.into())
            .map_err(|e| ClientError::NewBidError(format!("Failed to sign eip712 data: {:?}", e)))
    }

    /// Generates adapter calldata for executing an opportunity.
    ///
    /// # Arguments
    ///
    /// * `opportunity` - The EVM opportunity structure.
    /// * `bid_params` - Bid parameters.
    /// * `wallet` - A `LocalWallet` object for signing transactions.
    ///
    /// # Returns
    ///
    /// * `Result<Bytes, ClientError>` - The calldata bytes for the opportunity adapter.
    ///
    /// # Errors
    ///
    /// Returns an error if signature generation or execution parameter creation fails.
    pub fn make_adapter_calldata(
        &self,
        opportunity: OpportunityEvm,
        bid_params: BidParamsEvm,
        wallet: LocalWallet,
    ) -> Result<Bytes, ClientError> {
        let signature =
            self.get_signature(opportunity.clone(), bid_params.clone(), wallet.clone())?;
        let params =
            self.make_opportunity_execution_params(opportunity, bid_params, wallet.address())?;

        let calldata = opportunity_adapter::ExecuteOpportunityCall::encode(
            opportunity_adapter::ExecuteOpportunityCall {
                params,
                signature: signature.to_vec().into(),
            },
        );

        Ok(calldata.into())
    }
}
