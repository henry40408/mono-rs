use crate::schema::scrapes;
use diesel::{PgConnection, RunQueryDsl};
use std::time::SystemTime;

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
        diesel::insert_into(dsl::scrapes)
            .values(self)
            .execute(conn)?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use crate::entities::NewScrape;
    use crate::{establish_connection, Scraper};
    use diesel::result::Error;
    use diesel::Connection;

    fn setup() {
        std::env::set_var(
            "DATABASE_URL",
            "postgres://postgres:@localhost/bk_development",
        );
    }

    #[tokio::test]
    async fn test_from_scraped_to_new_scrape() -> anyhow::Result<()> {
        setup();

        let scraper = Scraper::from_url("https://www.example.com");
        let scraped = scraper.scrape().await?;
        let new_scrape = NewScrape::from(scraped);

        let conn = establish_connection()?;
        conn.test_transaction::<_, Error, _>(|| {
            new_scrape.save(&conn)?;
            Ok(())
        });

        Ok(())
    }
}
