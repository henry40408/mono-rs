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

/// Document to be scraped
#[derive(Debug)]
pub struct NewDocument<'a> {
    url: &'a str,
}

impl<'a> NewDocument<'a> {
    /// Scrape document with URL
    pub fn from_url(url: &'a str) -> Self {
        Self { url }
    }

    /// Scrap HTML with URL
    pub fn scrape(&self) -> failure::Fallible<Document> {
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
            None => bail!("empty response"),
            Some(v) => match serde_json::from_value::<String>(v) {
                Err(_e) => bail!("failed to deserialize HTML"),
                Ok(h) => h,
            },
        };

        let title_ro = html_e.call_js_fn("function () { return document.title }", false)?;
        let title = match title_ro.value {
            None => bail!("empty title"),
            Some(v) => match serde_json::from_value::<String>(v) {
                Err(_e) => bail!("document has no title"),
                Ok(t) => t,
            },
        };

        Ok(Document {
            params: self,
            title,
            html,
        })
    }
}

/// Scraped document
#[derive(Debug)]
pub struct Document<'a> {
    params: &'a NewDocument<'a>,
    /// Document title
    pub title: String,
    /// Raw HTML document
    pub html: String,
}

#[cfg(test)]
mod test {
    use crate::NewDocument;

    #[test]
    fn test_scrape() -> failure::Fallible<()> {
        let new_doc = NewDocument::from_url("https://www.example.com");
        let doc = new_doc.scrape()?;
        assert_eq!("https://www.example.com", doc.params.url);
        assert!(doc.title.contains("Example Domain"));
        assert!(doc.html.contains("Example Domain"));
        Ok(())
    }
}
