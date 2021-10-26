use std::time::SystemTime;

use diesel::PgConnection;

use crate::schema::scrapes;

/// Scrape in database
#[derive(Debug, Queryable, Insertable)]
pub struct Scrape {
    /// Primary key
    pub id: i32,
    /// URL to be scraped
    pub url: String,
    /// Scrape with headless Chromium
    pub headless: bool,
    /// Actual content from URL
    pub content: Vec<u8>,
    /// When the URL is scraped
    pub created_at: SystemTime,
}

/// Search parameters on scrapes
#[derive(Debug, Default)]
pub struct SearchScrape {
    /// Search URL
    pub url: Option<String>,
}

impl Scrape {
    /// Search scrapes with parameters
    pub fn search(
        conn: &PgConnection,
        params: &SearchScrape,
    ) -> diesel::result::QueryResult<Vec<Scrape>> {
        use crate::schema::scrapes::dsl;
        use diesel::prelude::*;

        let mut query = dsl::scrapes.into_boxed();

        let url = params.url.as_ref().map(|u| format!("%{}%", u));
        if let Some(url) = url {
            query = query.filter(dsl::url.like(url));
        }

        query.load::<Scrape>(conn)
    }
}

/// New scrape to database
#[derive(Debug, Insertable)]
#[table_name = "scrapes"]
pub struct NewScrape {
    /// URL scraped
    pub url: String,
    /// Scrape with headless Chromium
    pub headless: bool,
    /// Actual content from URL
    pub content: Vec<u8>,
}

impl NewScrape {
    /// Save scrape
    pub fn save(&self, conn: &PgConnection) -> diesel::result::QueryResult<()> {
        use crate::schema::scrapes::dsl;
        use diesel::prelude::*;

        diesel::insert_into(dsl::scrapes)
            .values(self)
            .execute(conn)?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use diesel::result::Error;
    use diesel::{Connection, PgConnection};

    use crate::entities::{NewScrape, Scrape, SearchScrape};
    use crate::{establish_connection, Scraper};

    fn setup() -> anyhow::Result<PgConnection> {
        std::env::set_var(
            "DATABASE_URL",
            "postgres://postgres:@localhost/bk_development",
        );
        establish_connection()
    }

    #[tokio::test]
    async fn test_search() -> anyhow::Result<()> {
        let conn = setup()?;
        let scrapes = Scrape::search(&conn, &SearchScrape::default())?;
        assert_eq!(0, scrapes.len());
        Ok(())
    }

    #[tokio::test]
    async fn test_save_and_search() -> anyhow::Result<()> {
        let conn = setup()?;

        let scraper = Scraper::from_url("https://www.example.com");
        let scraped = scraper.scrape().await?;
        let new_scrape = NewScrape::from(scraped);

        conn.test_transaction::<_, Error, _>(|| {
            new_scrape.save(&conn)?;

            let mut params = SearchScrape::default();
            params.url = Some("example".into());

            let scrapes = Scrape::search(&conn, &params)?;
            assert_eq!(1, scrapes.len());

            Ok(())
        });

        Ok(())
    }
}
