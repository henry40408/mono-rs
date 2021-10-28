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

use bk::entities::{NewScrape, Scrape, SearchScrape};
use bk::{init_pool, migrate_database, PgPooledConnection, Scraped, Scraper};
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
    /// Scrape and save to database
    Save {
        #[structopt(long)]
        /// Scrape with headless Chromium?
        headless: bool,
        #[structopt(name = "URLS")]
        /// URLs to be scraped
        urls: Vec<String>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().ok();

    let pool = init_pool()?;
    let conn = pool.get()?;
    migrate_database(&conn)?;

    let commands = Commands::from_args();
    match commands {
        Commands::Scrape { ref urls, .. } => scrape_command(urls).await?,
        Commands::Search { ref url } => search_command(url).await?,
        Commands::Save { ref urls, headless } => save_command(urls, headless).await?,
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

    let pool = init_pool()?;
    let conn = pool.get()?;
    let scrapes = Scrape::search(&conn, &params)?;

    println!("total {}", scrapes.len());
    for scrape in scrapes {
        println!("{}", scrape.id);
    }

    Ok(())
}

async fn save_command(urls: &[String], headless: bool) -> anyhow::Result<()> {
    let pool = init_pool()?;
    let conn = pool.get()?;

    let mut tasks = vec![];
    for url in urls {
        tasks.push(save(&conn, url, headless));
    }

    let results = futures::future::join_all(tasks).await;
    for result in results {
        let _result = result?;
    }

    Ok(())
}

async fn save(conn: &PgPooledConnection, url: &str, headless: bool) -> anyhow::Result<()> {
    let mut scraper = Scraper::from_url(url);
    scraper.headless = headless;

    let scraped = scraper.scrape().await?;

    let new_scrape = NewScrape::from(scraped);
    new_scrape.save(conn)?;

    Ok(())
}

#[cfg(test)]
mod test {
    use crate::save;
    use bk::{init_pool, migrate_database, PgPooledConnection};
    use diesel::Connection;

    fn setup() -> anyhow::Result<PgPooledConnection> {
        std::env::set_var("DATABASE_URL", "postgres://postgres:@localhost/bk_test");
        let pool = init_pool()?;
        let conn = pool.get()?;
        migrate_database(&conn)?;
        Ok(conn)
    }

    #[tokio::test]
    async fn test_save_command() -> anyhow::Result<()> {
        let conn = setup()?;
        let url = "https://www.example.com";
        conn.begin_test_transaction()?;
        save(&conn, &url, false).await?;
        Ok(())
    }
}
