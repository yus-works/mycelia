use anyhow::{Context, Result, anyhow};
use regex::Regex;
use scraper::{Html, Selector};
use tracing::{error, info, instrument};

mod log;
mod graph;
mod visualizer;

#[tokio::main]
async fn main() -> Result<()> {
    log::setup_logging()?;

    info!("Starting application");

    visualizer::server::start().await?;

    // if let Err(e) = run().await {
    //     error!("Fatal error: {:?}", e);
    //     return Err(e);
    // }

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
    let resp = reqwest::get("http://localhost:8080/pages/linux.html")
        .await
        .context("Failed to connect")?
        .text()
        .await
        .context("Failed to parse text")?;

    let links = extract_links(&resp).await?;
    for l in links {
        println!("{}", l);
    }

    Ok(resp)
}

#[instrument(skip(body))]
async fn extract_links(body: &str) -> Result<Vec<String>> {
    let doc = Html::parse_document(body);
    let selector = Selector::parse(
        r#"
        a[href^="https://en.wikipedia.org/wiki/"]:not([href*="?"]):not([href*="action="])
    "#,
    );

    if selector.is_err() {
        return Err(anyhow!("failed to create selector"));
    }

    #[rustfmt::skip]
    let wiki_article_re = Regex::new(
        r"^https://en\.wikipedia\.org/wiki/([^:?#]+)(?:#[^?]*)?$"
    ).unwrap();

    let namespace_re = Regex::new(
        r"^(Category|Wikipedia|Special|Template|Help|Portal|Book|Draft|File|MediaWiki|Module|TimedText|User|Talk):"
    ).unwrap();

    Ok(doc
        .select(&selector.unwrap())
        .filter_map(|el| el.value().attr("href"))
        .filter(|href| {
            let group = wiki_article_re.captures(href);
            if group.is_none() {
                return false;
            };

            let article_name = &group.unwrap()[1]; // 1 as in \1
            !namespace_re.is_match(article_name)
        })
        .map(|s| s.to_owned())
        .collect::<Vec<String>>())
}
