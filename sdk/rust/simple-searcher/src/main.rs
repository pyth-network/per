use {
    anyhow::{
        anyhow,
        Result,
    },
    express_relay_client::Client,
};

#[tokio::main]
async fn main() -> Result<()> {
    let client = Client::try_new("http://127.0.0.1:9000", Some("test")).map_err(|e| {
        eprintln!("Failed to create client: {:?}", e);
        anyhow!("Failed to create client")
    })?;

    let opportunities = client.get_opportunities().await.map_err(|e| {
        eprintln!("Failed to get opportunities: {:?}", e);
        anyhow!("Failed to get opportunities")
    })?;

    println!("Opportunities: {:?}", opportunities.len());
    // if !opportunities.is_empty() {
    //     println!("First opportunity: {:?}", opportunities[0].creation_time());
    // }
    Ok(())
}
