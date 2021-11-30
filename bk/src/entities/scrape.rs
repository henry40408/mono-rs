use anyhow::{bail, Context};
use chrono::NaiveDateTime;
use diesel::SqliteConnection;
use std::collections::HashMap;
use std::fmt::{Display, Formatter};

use crate::entities::{last_insert_rowid, lower, Content, NewContent, User};
use crate::schema::scrapes;
use crate::Scraped;

/// Scrape
#[derive(Debug, Queryable)]
pub struct Scrape {
    /// Primary key
    pub id: i32,
    /// User ID
    pub user_id: i32,
    /// URL to be scraped
    pub url: String,
    /// Scrape with headless Chromium
    pub headless: bool,
    /// Searchable
    pub searchable: bool,
    /// Optional title
    pub title: Option<String>,
    /// When the URL is scraped
    pub created_at: NaiveDateTime,
}

impl Scrape {
    /// Find scrape with ID
    pub fn find(conn: &SqliteConnection, id: i32) -> anyhow::Result<Scrape> {
        use crate::schema::scrapes::dsl;
        use diesel::prelude::*;
        dsl::scrapes
            .find(id)
            .first(conn)
            .context("cannot find scrape with ID")
    }

    /// Search scrapes with parameters
    pub fn search(
        conn: &SqliteConnection,
        params: &mut SearchScrape,
    ) -> anyhow::Result<Vec<Scrape>> {
        use crate::schema::contents::dsl as contents_dsl;
        use crate::schema::scrapes::dsl;
        use crate::schema::users::dsl as users_dsl;
        use diesel::prelude::*;

        let mut query = dsl::scrapes.into_boxed();

        if let Some(url) = params.url {
            let needle = format!("%{}%", url.to_lowercase());
            query = query.filter(lower(dsl::url.nullable()).like(needle));
        }
        if let Some(title) = params.title {
            let needle = format!("%{}%", title.to_lowercase());
            query = query.filter(lower(dsl::title).like(needle));
        }

        if params.content.is_some() {
            let mut scrape_ids = vec![];
            let contents = Content::search(conn, params)?;
            for c in contents {
                scrape_ids.push(c.scrape_id);
            }
            query = query.filter(dsl::id.eq_any(scrape_ids));
        }

        let scrapes: Vec<Scrape> = query
            .load::<Scrape>(conn)
            .context("failed to search scrapes")?;

        if let Some(ref mut users) = params.users {
            let mut user_ids = vec![];
            for scrape in &scrapes {
                user_ids.push(scrape.user_id);
            }

            let us: Vec<User> = users_dsl::users
                .filter(users_dsl::id.eq_any(user_ids))
                .load::<User>(conn)
                .context("failed to load users")?;
            for u in us {
                users.insert(u.id, u);
            }
        }

        if let Some(ref mut contents) = params.contents {
            let scrape_ids: Vec<i32> = scrapes.iter().map(|s| s.id).collect();
            let cs: Vec<Content> = contents_dsl::contents
                .filter(contents_dsl::scrape_id.eq_any(scrape_ids))
                .load::<Content>(conn)
                .context("failed to load contents")?;
            for c in cs {
                contents.insert(c.scrape_id, c);
            }
        }

        Ok(scrapes)
    }

    /// Delete one scrape
    pub fn delete(conn: &SqliteConnection, id: i32) -> anyhow::Result<usize> {
        use crate::schema::scrapes::dsl;
        use diesel::prelude::*;

        diesel::delete(dsl::scrapes.filter(dsl::id.eq(id)))
            .execute(conn)
            .context("failed to delete scrape")
    }

    /// Show properties
    pub fn traits(&self) -> ScrapeTraits {
        ScrapeTraits {
            headless: self.headless,
            searchable: self.searchable,
        }
    }
}

/// Search parameters on scrapes
#[derive(Debug, Default)]
pub struct SearchScrape<'a> {
    /// Search URL
    pub url: Option<&'a str>,
    /// Search title
    pub title: Option<&'a str>,
    /// Search content
    pub content: Option<&'a str>,
    /// Users to be loaded
    pub users: Option<HashMap<i32, User>>,
    /// Contents to be loaded
    pub contents: Option<HashMap<i32, Content>>,
}

/// Traits of scrape e.g. headless? searchable?
#[derive(Clone, Copy, Debug)]
pub struct ScrapeTraits {
    /// Scrape with headless Chromium?
    headless: bool,
    /// Searchable with SQL syntax?
    searchable: bool,
}

impl Display for ScrapeTraits {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut properties = vec![];
        if self.headless {
            properties.push("headless");
        }
        if self.searchable {
            properties.push("searchable");
        }
        write!(f, "{}", properties.join(","))
    }
}

/// New scrape
#[derive(Debug)]
pub struct NewScrape<'a> {
    /// Overwrite if entry exists?
    pub force: bool,
    /// URL scraped
    pub url: &'a str,
    /// Optional user ID
    pub user_id: Option<i32>,
    /// Scrape with headless Chromium
    pub headless: bool,
    /// Optional title,
    pub title: Option<String>,
    /// Actual content from URL
    pub content: Vec<u8>,
    /// Searchable content
    pub searchable_content: Option<String>,
}

impl<'a> From<Scraped<'a>> for NewScrape<'a> {
    fn from(scraped: Scraped<'a>) -> Self {
        match scraped {
            Scraped::Document(d) => Self {
                force: d.params.force,
                user_id: d.params.user_id,
                url: d.params.url,
                headless: d.params.headless,
                title: Some(d.title),
                content: d.html.as_bytes().to_vec(),
                searchable_content: Some(d.html),
            },
            Scraped::Blob(b) => Self {
                force: b.params.force,
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

impl<'a> NewScrape<'a> {
    /// Save scrape
    pub fn save(&self, conn: &SqliteConnection) -> anyhow::Result<i32> {
        use crate::schema::scrapes::dsl;
        use diesel::prelude::*;

        let user_id = match self.user_id {
            None => bail!("user ID is required"),
            Some(i) => i,
        };

        conn.transaction(|| {
            if self.force {
                diesel::delete(dsl::scrapes.filter(dsl::url.eq(self.url))).execute(conn)?;
            }

            let new_scrape = StrictNewScrape {
                url: self.url,
                user_id,
                headless: self.headless,
                searchable: self.searchable_content.is_some(),
                title: self.title.as_deref(),
            };
            let row_id = new_scrape.save(conn)?;

            let new_content = NewContent {
                scrape_id: row_id,
                content: &self.content,
                searchable_content: self.searchable_content.as_deref(),
            };
            new_content.save(conn)?;

            Ok(row_id)
        })
    }
}

/// New scrape to database
#[derive(Debug, Insertable)]
#[table_name = "scrapes"]
pub struct StrictNewScrape<'a> {
    /// URL scraped
    pub url: &'a str,
    /// User ID
    pub user_id: i32,
    /// Scrape with headless Chromium
    pub headless: bool,
    /// Searchable
    pub searchable: bool,
    /// Optional title
    pub title: Option<&'a str>,
}

impl<'a> StrictNewScrape<'a> {
    fn save(&self, conn: &SqliteConnection) -> anyhow::Result<i32> {
        use crate::schema::scrapes::dsl;
        use diesel::prelude::*;

        diesel::insert_into(dsl::scrapes)
            .values(self)
            .execute(conn)
            .context("failed to save scrape")?;

        let row_id = diesel::select(last_insert_rowid).get_result::<i32>(conn)?;
        Ok(row_id)
    }
}
