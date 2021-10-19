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

use headless_chrome::Browser;

/// Scraped document
#[derive(Debug)]
pub struct Document<'a> {
    url: &'a str,
    title: String,
    body: String,
}

/// Scrap HTML with URL
pub fn scrape(url: &str) -> failure::Fallible<Document> {
    let browser = Browser::default()?;
    let tab = browser.wait_for_initial_tab()?;
    tab.navigate_to(url)?;

    let title_e = tab.wait_for_element("title")?;
    let title = title_e.get_inner_text()?;

    let body_e = tab.wait_for_element("html")?;
    let body = body_e.get_inner_text()?;
    Ok(Document { url, title, body })
}

#[cfg(test)]
mod test {
    use crate::scrape;

    #[test]
    fn test_scrape() -> failure::Fallible<()> {
        let doc = scrape("https://www.example.com")?;
        assert_eq!("https://www.example.com", doc.url);
        assert!(doc.title.contains("Example Domain"));
        assert!(doc.body.contains("Example Domain"));
        Ok(())
    }
}
