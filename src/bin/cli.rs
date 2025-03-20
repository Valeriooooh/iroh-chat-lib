use iroh_chat_cli::cli::start_cli;
use iroh_chat_cli::iroh_client::Iroh;
use std::sync::Arc;

#[cfg(target_os = "linux")]
const fn get_directory() -> &'static str{
    ".iroh-dir"
}

#[cfg(target_os = "android")]
const fn get_directory() -> &'static str{
    ".iroh-dir"
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let i = Arc::new(Iroh::new(get_directory().into()).await?);
    start_cli(i).await;
    Ok(())
}
