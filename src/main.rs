use anyhow::{Context, Result};
use tracing::{error, info, instrument};

mod log;

#[tokio::main]
async fn main() -> Result<()> {
    log::setup_logging()?;

    info!("Starting application");

    if let Err(e) = run().await {
        error!("Fatal error: {:?}", e);
        return Err(e);
    }

    info!("Shutting down gracefully");
    Ok(())
}

#[instrument]
async fn run() -> Result<()> {
    let data = fetch_data().await?;
    info!(records = data.len(), "Data fetched successfully");
    Ok(())
}

#[instrument]
async fn fetch_data() -> Result<String> {
    let resp = reqwest::get("http://localhost:8080/hello")
        .await
        .context("Failed to connect")?
        .text()
        .await
        .context("Failed to parse text")?;
    Ok(resp)
}
