use cli::start_cli;
use iroh_client::Iroh;
use std::sync::Arc;
mod client;
mod iroh_client;
mod message;
mod cli;


#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let i = Arc::new(Iroh::new(".iroh-dir".into()).await?);
    start_cli(i).await;
    Ok(())
}
