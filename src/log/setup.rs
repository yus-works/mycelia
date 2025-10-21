use anyhow::Result;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt};

pub fn setup_logging() -> Result<()> {
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
