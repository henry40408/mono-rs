#![deny(
    missing_docs,
    missing_debug_implementations,
    missing_copy_implementations,
    trivial_casts,
    trivial_numeric_casts,
    unsafe_code,
    unstable_features,
    unused_import_braces,
    unused_qualifications
)]

//! Bookmark or bucket service

use bk::scrape::Scrape;
use bk::{connect_database, NewScrape, Scraped};
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(about, author)]
enum Commands {
    /// Scrape web page with URL
    Scrape {
        #[structopt(long)]
        /// Scrape with headless Chromium?
        headless: bool,
        #[structopt(name = "URLS")]
        /// URLs to be scraped
        urls: Vec<String>,
    },
    /// Search URL in database
    Search {
        #[structopt(short, long)]
        /// URL to be searched
        url: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().ok();
    let commands = Commands::from_args();
    match commands {
        Commands::Scrape { ref urls, .. } => scrape_command(urls).await?,
        Commands::Search { ref url } => search_command(url).await?,
    }
    Ok(())
}

async fn scrape_command(urls: &[String]) -> anyhow::Result<()> {
    for url in urls {
        let new_doc = NewScrape::from_url(url);
        let scraped = new_doc.scrape().await?;
        if let Scraped::Document(ref doc) = scraped {
            println!("{}", doc.html);
        }
        if let Scraped::Blob(ref blob) = scraped {
            eprintln!(
                "binary content, MIME type = {}, content length = {}",
                blob.mime_type.mime_type(),
                blob.content.len()
            );
        }
    }
    Ok(())
}

async fn search_command(url_query: &str) -> anyhow::Result<()> {
    use bk::schema::scrapes::dsl::*;
    use diesel::prelude::*;

    let connection = connect_database()?;
    let like = format!("%{}%", url_query);

    let rows: Vec<Scrape> = scrapes
        .filter(url.like(like))
        .limit(1)
        .load::<Scrape>(&connection)?;
    println!("total {}", rows.len());
    for row in rows {
        println!("{}", row.id);
    }

    Ok(())
}
