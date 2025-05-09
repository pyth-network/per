use auction_server::server;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    server().await
}
