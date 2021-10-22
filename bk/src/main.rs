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

use bk::{NewScrape, Scraped};
use failure::Fallible;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(about, author)]
enum Commands {
    /// Scrape web page with URL
    Scrape {
        #[structopt(long)]
        /// Scrape with Headless Chrome?
        headless: bool,
        #[structopt(name = "URLS")]
        /// URLs to be scraped
        urls: Vec<String>,
    },
}

#[tokio::main]
async fn main() -> Fallible<()> {
    let commands = Commands::from_args();
    match commands {
        Commands::Scrape { urls, .. } => {
            for url in urls {
                let new_doc = NewScrape::from_url(&url);
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
        }
    }
    Ok(())
}
