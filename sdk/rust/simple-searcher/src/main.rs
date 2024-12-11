use {
    anyhow::{
        anyhow,
        Result,
    },
    clap::Parser,
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
        ethers::types::U256,
        evm::BidParamsEvm,
        Client,
        ClientConfig,
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


#[derive(Parser, Clone, Debug)]
pub struct RunOptions {
    /// The http url of the express relay server.
    #[arg(long = "server-url")]
    #[arg(env = "SERVER_URL")]
    pub server_url: String,

    /// EVM private key in hex format.
    #[arg(long = "private-key-evm")]
    #[arg(env = "PRIVATE_KEY_EVM")]
    pub private_key_evm: Option<String>,

    /// SVM private key in base58 format.
    #[arg(long = "private-key-svm")]
    #[arg(env = "PRIVATE_KEY_SVM")]
    pub private_key_svm: Option<String>,

    /// Chain ids to subscribe to.
    #[arg(long = "chain-ids", required = true)]
    #[arg(env = "CHAIN_IDS")]
    pub chains: Vec<String>,
}


async fn random() -> U256 {
    let mut rng = rand::thread_rng();
    U256::from(rng.gen::<u128>())
}

async fn handle_opportunity(
    ws_client: WsClient,
    opportunity: Opportunity,
    private_key: String,
) -> Result<()> {
    // Assess the opportunity to see if it is worth bidding
    // For the sake of this example, we will always bid
    let bid = match opportunity {
        opportunity::Opportunity::Evm(opportunity) => {
            Client::new_bid(
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
            Client::new_bid(opportunity, 2, private_key).await
        }
    }
    .map_err(|e| {
        println!("Failed to create bid: {:?}", e);
        anyhow!("Failed to create bid: {:?}", e)
    })?;

    let result = ws_client.submit_bid(bid).await;
    match result {
        Ok(_) => println!("Bid submitted"),
        Err(e) => eprintln!("Failed to submit bid: {:?}", e),
    };
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: RunOptions = RunOptions::parse();
    let ws_url = if args.server_url.starts_with("http") {
        args.server_url.replace("http", "ws")
    } else {
        args.server_url.replace("https", "wss")
    };

    let client = Client::try_new(ClientConfig {
        http_url: args.server_url,
        ws_url,
        api_key: Some("Test".to_string()),
    })
    .map_err(|e| {
        eprintln!("Failed to create client: {:?}", e);
        anyhow!("Failed to create client")
    })?;

    let opportunities = client
        .get_opportunities(Some(GetOpportunitiesQueryParams {
            chain_id:       Some(args.chains[0].clone()),
            mode:           OpportunityMode::Historical,
            permission_key: None,
            limit:          100,
            from_time:      Some(OffsetDateTime::now_utc() - Duration::days(1)),
        }))
        .await
        .map_err(|e| {
            eprintln!("Failed to get opportunities: {:?}", e);
            anyhow!("Failed to get opportunities")
        })?;

    println!("Opportunities: {:?}", opportunities.len());

    let ws_client = client.connect_websocket().await.map_err(|e| {
        eprintln!("Failed to connect websocket: {:?}", e);
        anyhow!("Failed to connect websocket")
    })?;

    ws_client.chain_subscribe(args.chains).await.map_err(|e| {
        eprintln!("Failed to subscribe chains: {:?}", e);
        anyhow!("Failed to subscribe chains")
    })?;

    let mut stream = ws_client.get_update_stream();
    let mut block_hash_map = HashMap::new();
    while let Some(update) = stream.next().await {
        let update = match update {
            Ok(update) => update,
            Err(e) => {
                // The stream is fallen behind
                eprintln!("The stream is fallen behind: {:?}", e);
                continue;
            }
        };

        match update {
            ServerUpdateResponse::NewOpportunity { opportunity } => {
                let private_key = match opportunity {
                    Opportunity::Evm(_) => {
                        println!("EVM opportunity Received");
                        args.private_key_evm.clone()
                    }
                    Opportunity::Svm(_) => {
                        println!("SVM opportunity Received");
                        args.private_key_svm.clone()
                    }
                };

                match private_key {
                    Some(private_key) => {
                        tokio::spawn(handle_opportunity(
                            ws_client.clone(),
                            opportunity,
                            private_key,
                        ));
                    }
                    None => {
                        eprintln!("Private key not provided");
                    }
                }
            }
            ServerUpdateResponse::SvmChainUpdate { update } => {
                block_hash_map.insert(update.chain_id.clone(), update.blockhash);
                println!("SVM chain");
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
