use anyhow::{Context, Result};
use tracing::{error, info, instrument, warn};
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
    setup_logging()?;

    info!("Starting application");

    if let Err(e) = run().await {
        error!("Fatal error: {:?}", e);
        return Err(e);
    }

    info!("Shutting down gracefully");
    Ok(())
}

fn setup_logging() -> Result<()> {
    std::fs::create_dir_all("logs")?;

    #[rustfmt::skip]
    let file_appender = RollingFileAppender::new(
        Rotation::DAILY, "logs", "app.log"
    );

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,reqwest=debug".into()),
        )
        .with(
            fmt::layer()
                .with_writer(std::io::stdout)
                .with_span_events(fmt::format::FmtSpan::CLOSE),
        )
        .with(
            fmt::layer()
                .with_writer(file_appender)
                .with_ansi(false)
                .with_span_events(fmt::format::FmtSpan::CLOSE),
        )
        .init();

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
