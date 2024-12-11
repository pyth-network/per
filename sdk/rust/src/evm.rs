use {
    crate::ClientError,
    ethers::{
        contract::abigen,
        providers::{
            Http,
            Provider,
        },
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
    std::sync::Arc,
};

abigen!(
    OpportunityAdapter,
    "../../contracts/evm/out/OpportunityAdapter.sol/OpportunityAdapter.json";
    AdapterFactory,
    "../../contracts/evm/out/OpportunityAdapterFactory.sol/OpportunityAdapterFactory.json"
);
abigen!(ERC20, "../../contracts/evm/out/ERC20.sol/ERC20.json");
abigen!(WETH9, "../../contracts/evm/out/WETH9.sol/WETH9.json");

abigen!(
    ExpressRelay,
    "../../contracts/evm/out/ExpressRelay.sol/ExpressRelay.json"
);

#[derive(Debug, Clone)]
pub struct BidParamsEvm {
    pub amount:   ethers::types::U256,
    pub deadline: ethers::types::U256,
    pub nonce:    ethers::types::U256,
}

pub struct Config {
    pub weth:                     Address,
    pub adapter_factory_contract: Address,
    pub express_relay_contract:   Address,
    pub permit2:                  Address,
    pub adapter_bytecode_hash:    [u8; 32],
    pub chain_id_num:             u64,
}

pub fn get_config(chain_id: &str) -> Result<Config, ClientError> {
    match chain_id {
        "development" => Ok(Config {
            weth:                     "0x5FC8d32690cc91D4c39d9d3abcBD16989F875707"
                .parse()
                .expect("Invalid Ethereum address"),
            permit2:                  "0x8A791620dd6260079BF849Dc5567aDC3F2FdC318"
                .parse()
                .expect("Invalid Ethereum address"),
            adapter_factory_contract: "0x610178dA211FEF7D417bC0e6FeD39F05609AD788"
                .parse()
                .expect("Invalid Ethereum address"),
            express_relay_contract:   "0xa513E6E4b8f2a923D98304ec87F64353C4D5C853"
                .parse()
                .expect("Invalid Ethereum address"),
            adapter_bytecode_hash:    hex::decode(
                "0xd53b8e32ab2ecba07c3e3a17c3c5e492c62e2f7051b89e5154f52e6bfeb0e38f",
            )
            .expect("Invalid bytecode hash")
            .try_into()
            .expect("Invalid bytecode hash length"),
            chain_id_num:             31337,
        }),
        _ => Err(ClientError::ChainNotSupported),
    }
}

pub fn make_permitted_tokens(
    opportunity: OpportunityEvm,
    bid_params: BidParamsEvm,
) -> Result<Vec<TokenPermissions>, ClientError> {
    let config = get_config(opportunity.get_chain_id())?;
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

pub fn make_opportunity_execution_params(
    opportunity: OpportunityEvm,
    bid_params: BidParamsEvm,
    executor: Address,
) -> Result<ExecutionParams, ClientError> {
    let params = get_params(opportunity.clone());
    Ok(ExecutionParams {
        permit:  PermitBatchTransferFrom {
            permitted: make_permitted_tokens(opportunity, bid_params.clone())?,
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
        message: serde_json::from_value(data).expect("Failed to parse data for eip712 typed data"),
    }
}

fn get_signature(
    opportunity: OpportunityEvm,
    bid_params: BidParamsEvm,
    wallet: LocalWallet,
) -> Result<Signature, ClientError> {
    let config = get_config(opportunity.get_chain_id())?;
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

    let params = make_opportunity_execution_params(
        opportunity.clone(),
        bid_params.clone(),
        wallet.address(),
    )?;

    let typed_data: eip712::TypedData =
        get_typed_data(params.clone(), executor_adapter_address, eip_712_domain);
    let hashed_data = typed_data
        .encode_eip712()
        .map_err(|e| ClientError::NewBidError(format!("Failed to encode eip712 data: {:?}", e)))?;

    wallet
        .sign_hash(hashed_data.into())
        .map_err(|e| ClientError::NewBidError(format!("Failed to sign eip712 data: {:?}", e)))
}

pub fn make_adapter_calldata(
    opportunity: OpportunityEvm,
    bid_params: BidParamsEvm,
    wallet: LocalWallet,
) -> Result<Bytes, ClientError> {
    let config = get_config(opportunity.get_chain_id())?;
    let adapter_contract = config.adapter_factory_contract;
    let signature = get_signature(opportunity.clone(), bid_params.clone(), wallet.clone())?;
    let execution_params =
        make_opportunity_execution_params(opportunity, bid_params, wallet.address())?;

    let provider = Provider::<Http>::try_from("https://eth.llamarpc.com")
        .map_err(|e| ClientError::NewBidError(format!("Failed to create provider: {:?}", e)))?;
    let calldata = OpportunityAdapter::new(adapter_contract, Arc::new(provider))
        .execute_opportunity(execution_params, signature.to_vec().into())
        .calldata()
        .ok_or(ClientError::NewBidError(
            "Failed to generate calldata for opportunity adapter".to_string(),
        ))?;

    Ok(calldata)
}

pub fn get_params(opportunity: OpportunityEvm) -> OpportunityCreateV1Evm {
    let OpportunityParamsEvm::V1(OpportunityParamsV1Evm(params)) = opportunity.params;
    params
}
