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
        ethers::types::U256,
        evm::BidParamsEvm,
        ChainId,
        Client,
        ClientConfig,
        WsClient,
    },
    rand::Rng,
    std::{
        collections::HashMap,
        sync::Arc,
    },
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

async fn handle_opportunity(ws_client: Arc<WsClient>, opportunity: Opportunity) -> Result<()> {
    let bid = match opportunity {
        opportunity::Opportunity::Evm(opportunity) => {
            // Assess opportunity
            Client::new_bid(
                opportunity,
                BidParamsEvm {
                    amount:   U256::from(100),
                    nonce:    random().await,
                    deadline: U256::from(
                        (OffsetDateTime::now_utc() + Duration::days(1)).unix_timestamp(),
                    ),
                },
            )
            .await
        }
        opportunity::Opportunity::Svm(opportunity) => Client::new_bid(opportunity, 2).await,
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
    let client = Client::try_new(ClientConfig {
        http_url: "http://127.0.0.1:9000".to_string(),
        ws_url:   "ws://127.0.0.1:9000".to_string(),
        api_key:  Some("Test".to_string()),
    })
    .map_err(|e| {
        eprintln!("Failed to create client: {:?}", e);
        anyhow!("Failed to create client")
    })?;

    let opportunities = client
        .get_opportunities(Some(GetOpportunitiesQueryParams {
            chain_id:       Some(ChainId::DevelopmentSvm.to_string()),
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

    let ws_client = Arc::new(client.connect_websocket().await.map_err(|e| {
        eprintln!("Failed to connect websocket: {:?}", e);
        anyhow!("Failed to connect websocket")
    })?);

    ws_client
        .chain_subscribe(vec![ChainId::DevelopmentEvm, ChainId::DevelopmentSvm])
        .await
        .map_err(|e| {
            eprintln!("Failed to subscribe chains: {:?}", e);
            anyhow!("Failed to subscribe chains")
        })?;

    let mut stream = ws_client.update_stream.write().await;
    let mut block_hash_map = HashMap::new();
    while let Some(update) = stream.next().await {
        match update {
            ServerUpdateResponse::NewOpportunity { opportunity } => {
                println!("New opportunity: {:?}", opportunity);
                tokio::spawn(handle_opportunity(ws_client.clone(), opportunity));
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
