use {
    anyhow::{
        anyhow,
        Result,
    },
    express_relay_client::{
        api_types::{
            bid::{
                BidCancel,
                BidCancelSvm,
                BidId,
                BidStatus,
                BidStatusSvm,
            },
            opportunity::{
                self,
                GetOpportunitiesQueryParams,
                Opportunity,
                OpportunityMode,
                OpportunityParamsSvm,
                OpportunityParamsV1ProgramSvm,
                QuoteTokens,
            },
            ws::ServerUpdateResponse,
            SvmChainUpdate,
        },
        ethers::{
            signers::LocalWallet,
            types::U256,
        },
        evm,
        solana_sdk::{
            compute_budget::ComputeBudgetInstruction,
            signature::Keypair,
            signer::Signer,
        },
        svm,
        Client,
        WsClient,
    },
    rand::Rng,
    spl_associated_token_account::instruction::create_associated_token_account_idempotent,
    std::{
        collections::HashMap,
        sync::Arc,
    },
    time::{
        Duration,
        OffsetDateTime,
    },
    tokio::sync::RwLock,
    tokio_stream::StreamExt,
};

async fn random() -> U256 {
    let mut rng = rand::thread_rng();
    U256::from(rng.gen::<u128>())
}

#[derive(Clone)]
pub struct SimpleSearcher {
    client:          Client,
    ws_client:       WsClient,
    private_key_evm: Option<String>,
    private_key_svm: Option<String>,
    chain_ids:       Vec<String>,
    svm_update_map:  HashMap<String, SvmChainUpdate>,
    svm_client:      Option<Arc<svm::Svm>>,
    bid_chain_id:    Arc<RwLock<HashMap<BidId, String>>>,
}

const SVM_BID_AMOUNT: u64 = 10_000_000;
const EVM_BID_AMOUNT: i128 = 5_000_000_000_000_000_000_i128;

impl SimpleSearcher {
    pub async fn try_new(
        client: Client,
        chain_ids: Vec<String>,
        private_key_evm: Option<String>,
        private_key_svm: Option<String>,
        svm_rpc_url: Option<String>,
    ) -> Result<Self> {
        if let Some(private_key) = private_key_evm.clone() {
            private_key
                .parse::<LocalWallet>()
                .map_err(|e| anyhow!("Invalid evm private key: {}", e))?;
        }

        if let Some(private_key) = private_key_svm.clone() {
            Keypair::from_base58_string(private_key.as_str());
        }

        let svm_client = svm_rpc_url.map(|url| Arc::new(svm::Svm::new(url.clone())));
        let ws_client = client.connect_websocket().await.map_err(|e| {
            eprintln!("Failed to connect websocket: {:?}", e);
            anyhow!("Failed to connect websocket")
        })?;

        Ok(Self {
            client,
            ws_client,
            private_key_evm,
            private_key_svm,
            chain_ids,
            svm_update_map: HashMap::new(),
            svm_client,
            bid_chain_id: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    async fn bid_on_existing_opps(&self) -> Result<()> {
        let opportunities = self
            .client
            .get_opportunities(Some(GetOpportunitiesQueryParams {
                chain_id:       Some(self.chain_ids[0].clone()),
                mode:           OpportunityMode::Live,
                permission_key: None,
                limit:          100,
                from_time:      Some(OffsetDateTime::now_utc() - Duration::days(1)),
            }))
            .await
            .map_err(|e| {
                eprintln!("Failed to get opportunities: {:?}", e);
                anyhow!("Failed to get opportunities")
            })?;
        opportunities.iter().for_each(|opportunity| {
            let (searcher, opportunity) = (self.clone(), opportunity.clone());
            tokio::spawn(async move {
                searcher.handle_opportunity(opportunity).await;
            });
        });
        Ok(())
    }

    async fn handle_opportunity(&self, opportunity: Opportunity) {
        // Assess the opportunity to see if it is worth bidding
        // For the sake of this example, we will always bid
        let private_key = match opportunity {
            Opportunity::Evm(_) => {
                println!("EVM opportunity Received");
                self.private_key_evm.clone()
            }
            Opportunity::Svm(_) => {
                println!("SVM opportunity Received");
                self.private_key_svm.clone()
            }
        };

        match private_key {
            Some(private_key) => {
                if let Err(e) = self.submit_bid(opportunity, private_key).await {
                    eprintln!("Failed to submit opportunity: {:?}", e);
                }
            }
            None => {
                eprintln!("Private key not provided");
            }
        }
    }

    async fn submit_bid(&self, opportunity: Opportunity, private_key: String) -> Result<()> {
        let deadline = (OffsetDateTime::now_utc() + Duration::days(1)).unix_timestamp();
        let bid = match opportunity.clone() {
            opportunity::Opportunity::Evm(opportunity) => {
                let wallet = private_key.parse::<LocalWallet>().map_err(|e| {
                    eprintln!("Failed to parse evm private key: {:?}", e);
                    anyhow!("Failed to parse evm private key")
                })?;
                let bid_params = evm::BidParams {
                    amount:   U256::from(EVM_BID_AMOUNT),
                    nonce:    random().await,
                    deadline: U256::from(deadline),
                };
                self.client
                    .new_bid(opportunity, evm::NewBidParams { bid_params, wallet })
                    .await
            }
            opportunity::Opportunity::Svm(opportunity) => {
                let svm_update = self
                    .svm_update_map
                    .get(opportunity.get_chain_id())
                    .cloned()
                    .ok_or(anyhow!("Block hash not found"))?;
                let payer = Keypair::from_base58_string(private_key.as_str());
                // This limit assumes no other custom instructions exist in the transaction, you may need to adjust
                // this limit depending on your integration
                let compute_limit_ix = ComputeBudgetInstruction::set_compute_unit_limit(300_000);
                let fee_ix = ComputeBudgetInstruction::set_compute_unit_price(
                    svm_update.latest_prioritization_fee,
                );
                if self.svm_client.is_none() {
                    return Err(anyhow!("SVM RPC client not provided"));
                }
                let svm_client = self
                    .svm_client
                    .as_ref()
                    .expect("SVM RPC client not provided");
                let metadata = svm_client
                    .get_express_relay_metadata(opportunity.get_chain_id().clone())
                    .await
                    .map_err(|e| anyhow!("Failed to get express relay metadata: {:?}", e))?;

                let OpportunityParamsSvm::V1(params) = opportunity.params.clone();
                match params.program {
                    OpportunityParamsV1ProgramSvm::Limo {
                        order: _order,
                        order_address: _order_address,
                        slot: _slot,
                    } => {
                        // TODO EXTRACT ROUTER DATA FROM LIMONADE SDK
                        // self.client
                        //     .new_bid(
                        //         opportunity.clone(),
                        //         svm::NewBidParams {
                        //             amount: SVM_BID_AMOUNT,
                        //             deadline,
                        //             block_hash: svm_update.blockhash,
                        //             instructions: vec![compute_limit_ix, fee_ix],
                        //             payer: payer.pubkey(),
                        //             slot: Some(slot),
                        //             searcher: payer.pubkey(),
                        //             fee_receiver_relayer: metadata
                        //                 .fee_receiver_relayer
                        //                 .to_bytes()
                        //                 .into(),
                        //             signers: vec![payer],
                        //             relayer_signer: metadata
                        //                 .relayer_signer
                        //                 .to_bytes()
                        //                 .into(),
                        //             program_params: svm::ProgramParams::Limo(
                        //                 svm::ProgramParamsLimo {
                        //                     router:         Pubkey::from_str(
                        //                         "FjgAP9DWiSmULyUKwMrMTTfwdGJeMz22Bcibzq4ijzPR",
                        //                     )
                        //                     .expect("Failed to parse pubkey"),
                        //                     permission:     order_address,
                        //                 },
                        //             ),
                        //         },
                        //     )
                        //     .await
                        Err(express_relay_client::ClientError::NewBidError(
                            "Limo not supported yet".to_string(),
                        ))
                    }
                    OpportunityParamsV1ProgramSvm::Swap { tokens, .. } => {
                        let (user_token, token_program_user) = match tokens.tokens {
                            QuoteTokens::SearcherTokenSpecified { user_token, .. } => {
                                (user_token, tokens.token_program_user)
                            }
                            QuoteTokens::UserTokenSpecified { user_token, .. } => {
                                (user_token, tokens.token_program_user)
                            }
                        };
                        let create_ata_ix = create_associated_token_account_idempotent(
                            &payer.pubkey(),
                            &payer.pubkey(),
                            &user_token,
                            &token_program_user,
                        );
                        self.client
                            .new_bid(
                                opportunity.clone(),
                                svm::NewBidParams {
                                    amount: SVM_BID_AMOUNT,
                                    deadline,
                                    block_hash: svm_update.blockhash,
                                    instructions: vec![compute_limit_ix, fee_ix, create_ata_ix],
                                    payer: payer.pubkey(),
                                    slot: None,
                                    searcher: payer.pubkey(),
                                    fee_receiver_relayer: metadata.fee_receiver_relayer,
                                    signers: vec![payer],
                                    relayer_signer: metadata.relayer_signer,
                                    program_params: svm::ProgramParams::Swap(
                                        svm::ProgramParamsSwap {},
                                    ),
                                },
                            )
                            .await
                    }
                }
            }
        }
        .map_err(|e| {
            eprintln!("Failed to create bid: {:?}", e);
            anyhow!("Failed to create bid: {:?}", e)
        })?;

        let result = self.ws_client.submit_bid(bid).await;
        match result {
            Ok(bid_result) => {
                self.bid_chain_id
                    .write()
                    .await
                    .insert(bid_result.id, opportunity.get_chain_id().clone());
                println!("Bid submitted");
            }
            Err(e) => eprintln!("Failed to submit bid: {:?}", e),
        };
        Ok(())
    }

    pub async fn run(&mut self) -> Result<()> {
        self.bid_on_existing_opps().await?;

        self.ws_client
            .chain_subscribe(self.chain_ids.clone())
            .await
            .map_err(|e| {
                eprintln!("Failed to subscribe chains: {:?}", e);
                anyhow!("Failed to subscribe chains")
            })?;

        let mut stream = self.ws_client.get_update_stream();
        while let Some(update) = stream.next().await {
            let update = match update {
                Ok(update) => update,
                Err(e) => {
                    eprintln!("The stream is fallen behind: {:?}", e);
                    continue;
                }
            };

            match update {
                ServerUpdateResponse::NewOpportunity { opportunity } => {
                    let searcher = self.clone();
                    tokio::spawn(async move {
                        searcher.handle_opportunity(opportunity).await;
                    });
                }
                ServerUpdateResponse::SvmChainUpdate { update } => {
                    self.svm_update_map
                        .insert(update.chain_id.clone(), update.clone());
                    println!("SVM chain update: {:?}", update);
                }
                ServerUpdateResponse::RemoveOpportunities { opportunity_delete } => {
                    println!("Remove opportunities: {:?}", opportunity_delete);
                }
                ServerUpdateResponse::BidStatusUpdate { status } => {
                    println!("Bid status update: {:?}", status);
                    // It's possible to cancel bids with status awaiting_signature
                    // Doing it here randomly for demonstration purposes
                    if let BidStatus::Svm(BidStatusSvm::AwaitingSignature { .. }) =
                        status.bid_status
                    {
                        if let Some(chain_id) =
                            self.bid_chain_id.read().await.get(&status.id).cloned()
                        {
                            if rand::thread_rng().gen::<f64>() < 1.0 / 3.0 {
                                let result = self
                                    .ws_client
                                    .cancel_bid(BidCancel::Svm(BidCancelSvm {
                                        chain_id,
                                        bid_id: status.id,
                                    }))
                                    .await;
                                match result {
                                    Ok(_) => println!("Bid cancelled"),
                                    Err(e) => eprintln!("Failed to cancel bid: {:?}", e),
                                };
                            }
                        }
                    }
                }
            }
        }

        println!("Websocket closed");
        Ok(())
    }
}
