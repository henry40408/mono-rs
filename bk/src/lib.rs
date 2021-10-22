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

use failure::bail;
use headless_chrome::Browser;
use scraper::{Html, Selector};

/// Parameters for scrape
#[derive(Debug)]
pub struct NewScrape<'a> {
    url: &'a str,
    /// Scrape with headless Chrome
    pub headless: bool,
}

impl<'a> NewScrape<'a> {
    /// Scrape blob or document with URL
    pub fn from_url(url: &'a str) -> Self {
        Self {
            url,
            headless: false,
        }
    }

    /// Scrap document or blob w/ or w/o headless Chromium
    pub async fn scrape(&'a self) -> failure::Fallible<Scraped<'a>> {
        if self.headless {
            self.scrape_with_headless_chrome()
        } else {
            self.scrape_wo_headless_chrome().await
        }
    }

    fn scrape_with_headless_chrome(&self) -> failure::Fallible<Scraped> {
        let browser = Browser::default()?;
        let tab = browser.wait_for_initial_tab()?;
        tab.navigate_to(self.url)?;

        tab.wait_until_navigated()?; // wait for initial rendering

        let html_e = tab.wait_for_element("html")?;

        let html_ro = html_e.call_js_fn(
            "function () { return document.querySelector('html').outerHTML }",
            false,
        )?;
        let html = match html_ro.value {
            None => bail!("empty HTML document"),
            Some(v) => match serde_json::from_value::<String>(v) {
                Err(_e) => bail!("failed to deserialize HTML"),
                Ok(h) => h,
            },
        };

        let title_ro = html_e.call_js_fn("function () { return document.title }", false)?;
        let title = match title_ro.value {
            None => bail!("no title element found"),
            Some(v) => match serde_json::from_value::<String>(v) {
                Err(_e) => bail!("failed to deserialize document title"),
                Ok(t) => t,
            },
        };

        Ok(Scraped::Document(Document {
            params: self,
            title,
            html,
        }))
    }

    async fn scrape_wo_headless_chrome(&'a self) -> failure::Fallible<Scraped<'a>> {
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
    params: &'a NewScrape<'a>,
    /// Inferred MIME type
    pub mime_type: infer::Type,
    /// Blob content
    pub content: Vec<u8>,
}

/// Scraped document
#[derive(Debug)]
pub struct Document<'a> {
    params: &'a NewScrape<'a>,
    /// Document title
    pub title: String,
    /// Raw HTML document
    pub html: String,
}

#[cfg(test)]
mod test {
    use crate::{NewScrape, Scraped};

    #[tokio::test]
    async fn test_scrape_with_headless_chrome() -> failure::Fallible<()> {
        let mut new_doc = NewScrape::from_url("https://www.example.com");
        new_doc.headless = true;

        let s = new_doc.scrape().await?;
        assert!(matches!(s, Scraped::Document(_)));

        if let Scraped::Document(doc) = s {
            assert_eq!("https://www.example.com", doc.params.url);
            assert!(doc.title.contains("Example Domain"));
            assert!(doc.html.contains("Example Domain"));
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_scrape_wo_headless_chrome() -> failure::Fallible<()> {
        let new_doc = NewScrape::from_url("https://www.example.com");

        let s = new_doc.scrape().await?;
        assert!(matches!(s, Scraped::Document(_)));

        if let Scraped::Document(doc) = s {
            assert_eq!("https://www.example.com", doc.params.url);
            assert!(doc.title.contains("Example Domain"));
            assert!(doc.html.contains("Example Domain"));
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_scrape_image() -> failure::Fallible<()> {
        let new_doc = NewScrape::from_url("https://picsum.photos/1");

        let s = new_doc.scrape().await?;
        assert!(matches!(s, Scraped::Blob(_)));

        if let Scraped::Blob(blob) = s {
            assert_eq!("https://picsum.photos/1", blob.params.url);
            assert!(blob.content.len() > 0);
        }

        Ok(())
    }
}
