pub mod cli;
pub mod client;
pub mod iroh_client;
pub mod message;

mod messages;
mod sample_functions;

rinf::write_interface!();

#[tokio::main(flavor = "current_thread")]
async fn main() {
    // Spawn concurrent tasks.
    // Always use non-blocking async functions like `tokio::fs::File::open`.
    // If you must use blocking code, use `tokio::task::spawn_blocking`
    // or the equivalent provided by your async library.
    tokio::spawn(sample_functions::communicate());

    // Keep the main function running until Dart shutdown.
    rinf::dart_shutdown().await;
}
