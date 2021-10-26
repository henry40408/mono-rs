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

use bk::entities::{Scrape, SearchScrape};
use bk::{establish_connection, migrate_database, Scraped, Scraper};
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
        url: Option<String>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().ok();

    let conn = establish_connection()?;
    migrate_database(&conn)?;

    let commands = Commands::from_args();
    match commands {
        Commands::Scrape { ref urls, .. } => scrape_command(urls).await?,
        Commands::Search { ref url } => search_command(url).await?,
    }
    Ok(())
}

async fn scrape_command(urls: &[String]) -> anyhow::Result<()> {
    for url in urls {
        let new_doc = Scraper::from_url(url);
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

async fn search_command(url: &Option<String>) -> anyhow::Result<()> {
    let params = SearchScrape {
        url: url.to_owned(),
    };

    let connection = establish_connection()?;
    let scrapes = Scrape::search(&connection, &params)?;

    println!("total {}", scrapes.len());
    for scrape in scrapes {
        println!("{}", scrape.id);
    }

    Ok(())
}
