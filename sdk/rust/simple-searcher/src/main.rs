use {
    anyhow::{
        anyhow,
        Result,
    },
    express_relay_client::Client,
};

fn main() -> Result<()> {
    let client = Client::try_new("http://127.0.0.1:9000", Some("test")).map_err(|e| {
        eprintln!("Failed to create client: {:?}", e);
        anyhow!("Failed to create client")
    })?;
    client.test();
    Ok(())
}
