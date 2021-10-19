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

/// Scrap HTML with URL
pub fn scrape(url: &str) -> failure::Fallible<String> {
    let browser = Browser::default()?;
    let tab = browser.wait_for_initial_tab()?;
    tab.navigate_to(url)?;

    let body = tab.wait_for_element("html")?;
    let s = body.get_inner_text()?;
    Ok(s.trim().to_string())
}

#[cfg(test)]
mod test {
    use crate::scrape;

    #[test]
    fn test_scrape() -> failure::Fallible<()> {
        let s = scrape("https://www.example.com")?;
        assert!(s.contains("Example Domain"));
        Ok(())
    }
}
