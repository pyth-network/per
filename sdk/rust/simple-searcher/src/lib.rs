use {
    anyhow::{
        anyhow,
        Result,
    },
    express_relay_client::{
        api_types::{
            opportunity::{
                self,
                GetOpportunitiesQueryParams,
                Opportunity,
                OpportunityMode,
            },
            ws::ServerUpdateResponse,
        },
        ethers::{
            signers::LocalWallet,
            types::U256,
        },
        evm::BidParamsEvm,
        Client,
        WsClient,
    },
    rand::Rng,
    std::collections::HashMap,
    time::{
        Duration,
        OffsetDateTime,
    },
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
}

impl SimpleSearcher {
    pub async fn try_new(
        client: Client,
        chain_ids: Vec<String>,
        private_key_evm: Option<String>,
        private_key_svm: Option<String>,
    ) -> Result<Self> {
        if let Some(private_key) = private_key_evm.clone() {
            private_key
                .parse::<LocalWallet>()
                .map_err(|e| anyhow!("Invalid evm private key: {}", e))?;
        }

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
                if let Err(e) = self.submit_opportunity(opportunity, private_key).await {
                    eprintln!("Failed to submit opportunity: {:?}", e);
                }
            }
            None => {
                eprintln!("Private key not provided");
            }
        }
    }

    async fn submit_opportunity(
        &self,
        opportunity: Opportunity,
        private_key: String,
    ) -> Result<()> {
        let bid = match opportunity {
            opportunity::Opportunity::Evm(opportunity) => {
                self.client
                    .new_bid(
                        opportunity,
                        BidParamsEvm {
                            amount:   U256::from(5_000_000_000_000_000_000_i128),
                            nonce:    random().await,
                            deadline: U256::from(
                                (OffsetDateTime::now_utc() + Duration::days(1)).unix_timestamp(),
                            ),
                        },
                        private_key,
                    )
                    .await
            }
            opportunity::Opportunity::Svm(opportunity) => {
                self.client.new_bid(opportunity, 2, private_key).await
            }
        }
        .map_err(|e| {
            eprintln!("Failed to create bid: {:?}", e);
            anyhow!("Failed to create bid: {:?}", e)
        })?;

        let result = self.ws_client.submit_bid(bid).await;
        match result {
            Ok(_) => println!("Bid submitted"),
            Err(e) => eprintln!("Failed to submit bid: {:?}", e),
        };
        Ok(())
    }

    pub async fn run(&self) -> Result<()> {
        self.bid_on_existing_opps().await?;

        self.ws_client
            .chain_subscribe(self.chain_ids.clone())
            .await
            .map_err(|e| {
                eprintln!("Failed to subscribe chains: {:?}", e);
                anyhow!("Failed to subscribe chains")
            })?;

        let mut stream = self.ws_client.get_update_stream();
        let mut block_hash_map = HashMap::new();
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
                    block_hash_map.insert(update.chain_id.clone(), update.blockhash);
                    println!("SVM chain update: {:?}", update);
                }
                ServerUpdateResponse::RemoveOpportunities { opportunity_delete } => {
                    println!("Remove opportunities: {:?}", opportunity_delete);
                }
                ServerUpdateResponse::BidStatusUpdate { status } => {
                    println!("Bid status update: {:?}", status);
                }
            }
        }

        println!("Websocket closed");
        Ok(())
    }
}
