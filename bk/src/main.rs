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

use bk::NewDocument;
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

fn main() -> Fallible<()> {
    let commands = Commands::from_args();
    match commands {
        Commands::Scrape { urls, .. } => {
            for url in urls {
                let new_doc = NewDocument::from_url(&url);
                let doc = new_doc.scrape()?;
                println!("{}", doc.html);
            }
        }
    }
    Ok(())
}
