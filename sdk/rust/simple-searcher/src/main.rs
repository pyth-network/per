use {
    anyhow::{
        anyhow,
        Result,
    },
    express_relay_client::{
        api_types::opportunity::{
            GetOpportunitiesQueryParams,
            OpportunityMode,
        },
        ChainId,
        Client,
        ClientConfig,
    },
    time::{
        Duration,
        OffsetDateTime,
    },
};

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

    let mut ws_client = client.connect_websocket().await.map_err(|e| {
        eprintln!("Failed to connect websocket: {:?}", e);
        anyhow!("Failed to connect websocket")
    })?;

    ws_client
        .chain_subscribe(vec![ChainId::DevelopmentEvm, ChainId::DevelopmentSvm])
        .await
        .map_err(|e| {
            eprintln!("Failed to subscribe chains: {:?}", e);
            anyhow!("Failed to subscribe chains")
        })?;

    Ok(())
}
