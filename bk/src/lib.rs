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

#[macro_use]
extern crate diesel;

#[macro_use]
extern crate diesel_migrations;

use crate::entities::NewScrape;
use anyhow::bail;
use diesel::{Connection, SqliteConnection};
use failure::ResultExt;
use headless_chrome::Browser;
use scraper::{Html, Selector};
use std::env;

#[allow(missing_docs)]
pub mod schema;

/// Database models
pub mod entities;

/// Prelude
pub mod prelude;

embed_migrations!();

/// Build SQLite connection with environment variable
pub fn connect_database() -> anyhow::Result<SqliteConnection> {
    let uri = env::var("DATABASE_URL").expect("DATABASE_URL is required");
    Ok(SqliteConnection::establish(&uri)?)
}

/// Run database migrations
pub fn migrate_database(
    conn: &SqliteConnection,
) -> Result<(), diesel_migrations::RunMigrationsError> {
    embedded_migrations::run(conn)
}

/// Parameters for scrape
#[derive(Debug)]
pub struct Scraper<'a> {
    url: &'a str,
    /// Optional user ID
    pub user_id: Option<i32>,
    /// Scrape with headless Chromium
    pub headless: bool,
}

impl<'a> Scraper<'a> {
    /// Scrape blob or document with URL
    pub fn from_url(url: &'a str) -> Self {
        Self {
            url,
            user_id: None,
            headless: false,
        }
    }

    /// Scrap document or blob w/ or w/o headless Chromium
    pub async fn scrape(&'a self) -> anyhow::Result<Scraped<'a>> {
        if self.headless {
            self.scrape_with_headless_chromium()
        } else {
            self.scrape_wo_headless_chromium().await
        }
    }

    fn scrape_with_headless_chromium(&self) -> anyhow::Result<Scraped> {
        let browser = Browser::default().compat()?;
        let tab = browser.wait_for_initial_tab().compat()?;

        tab.navigate_to(self.url).compat()?;

        // wait for initial rendering
        tab.wait_until_navigated().compat()?;

        let html_e = tab.wait_for_element("html").compat()?;

        let html_ro = html_e
            .call_js_fn(
                "function () { return document.querySelector('html').outerHTML }",
                false,
            )
            .compat()?;
        let html = match html_ro.value {
            None => bail!("empty HTML document"),
            Some(v) => serde_json::from_value::<String>(v)?,
        };

        let title_ro = html_e
            .call_js_fn("function () { return document.title }", false)
            .compat()?;
        let title = match title_ro.value {
            None => bail!("no title element found"),
            Some(v) => serde_json::from_value::<String>(v)?,
        };

        Ok(Scraped::Document(Document {
            params: self,
            title,
            html,
        }))
    }

    async fn scrape_wo_headless_chromium(&'a self) -> anyhow::Result<Scraped<'a>> {
        let res = reqwest::get(self.url).await?;
        let content = res.bytes().await?;

        if infer::is_image(&content) {
            let mime_type = match infer::get(&content) {
                None => bail!("unknown MIME type"),
                Some(t) => t,
            };
            Ok(Scraped::Blob(Blob {
                params: self,
                mime_type,
                content: content.to_vec(),
            }))
        } else {
            let html = String::from_utf8_lossy(&content).to_string();

            let parsed = Html::parse_document(&html);
            let selector = Selector::parse("title").unwrap();

            let title = match parsed.select(&selector).next() {
                None => bail!("no title element found"),
                Some(t) => t.text().collect::<Vec<_>>().join(""),
            };
            Ok(Scraped::Document(Document {
                params: self,
                title,
                html,
            }))
        }
    }
}

/// Scraped blob or document
#[derive(Debug)]
pub enum Scraped<'a> {
    /// e.g. Image
    Blob(Blob<'a>),
    /// e.g. HTML document
    Document(Document<'a>),
}

/// Scraped blob
#[derive(Debug)]
pub struct Blob<'a> {
    params: &'a Scraper<'a>,
    /// Inferred MIME type
    pub mime_type: infer::Type,
    /// Blob content
    pub content: Vec<u8>,
}

/// Scraped document
#[derive(Debug)]
pub struct Document<'a> {
    params: &'a Scraper<'a>,
    /// Document title
    pub title: String,
    /// Raw HTML document
    pub html: String,
}

impl<'a> From<Scraped<'a>> for NewScrape<'a> {
    fn from(scraped: Scraped<'a>) -> Self {
        match scraped {
            Scraped::Document(d) => Self {
                user_id: d.params.user_id,
                url: d.params.url,
                headless: d.params.headless,
                title: Some(d.title),
                content: d.html.as_bytes().to_vec(),
                searchable_content: Some(d.html),
            },
            Scraped::Blob(b) => Self {
                user_id: b.params.user_id,
                url: b.params.url,
                headless: b.params.headless,
                title: None,
                content: b.content.to_vec(),
                searchable_content: None,
            },
        }
    }
}

#[cfg(test)]
mod test {
    use crate::{Scraped, Scraper};

    #[tokio::test]
    async fn test_scrape_with_headless_chromium() -> anyhow::Result<()> {
        let mut scraper = Scraper::from_url("https://www.example.com");
        scraper.headless = true;

        let scraped = scraper.scrape().await?;
        assert!(matches!(scraped, Scraped::Document(_)));

        if let Scraped::Document(doc) = scraped {
            assert_eq!("https://www.example.com", doc.params.url);
            assert!(doc.title.contains("Example Domain"));
            assert!(doc.html.contains("Example Domain"));
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_scrape_wo_headless_chromium() -> anyhow::Result<()> {
        let scraper = Scraper::from_url("https://www.example.com");

        let scraped = scraper.scrape().await?;
        assert!(matches!(scraped, Scraped::Document(_)));

        if let Scraped::Document(doc) = scraped {
            assert_eq!("https://www.example.com", doc.params.url);
            assert!(doc.title.contains("Example Domain"));
            assert!(doc.html.contains("Example Domain"));
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_scrape_image() -> anyhow::Result<()> {
        let scraper = Scraper::from_url("https://picsum.photos/1");

        let scraped = scraper.scrape().await?;
        assert!(matches!(scraped, Scraped::Blob(_)));

        if let Scraped::Blob(blob) = scraped {
            assert_eq!("https://picsum.photos/1", blob.params.url);
            assert!(blob.content.len() > 0);
        }

        Ok(())
    }
}
