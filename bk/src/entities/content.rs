use anyhow::Context;
use chrono::NaiveDateTime;
use diesel::SqliteConnection;

use crate::entities::{last_insert_rowid, lower, SearchScrape};
use crate::schema::contents;

/// Scrapped content
#[derive(Debug, Queryable)]
pub struct Content {
    /// Primary key
    pub id: i32,
    /// Scrape ID,
    pub scrape_id: i32,
    /// Actual content from URL
    pub content: Vec<u8>,
    /// Optional searchable content, must be string
    pub searchable_content: Option<String>,
    /// Created at
    pub created_at: NaiveDateTime,
}

impl Content {
    /// Find scrapped content by scrape ID
    pub fn find_by_scrape_id(conn: &SqliteConnection, scrape_id: i32) -> anyhow::Result<Content> {
        use crate::schema::contents::dsl;
        use diesel::prelude::*;
        dsl::contents
            .filter(dsl::scrape_id.eq(scrape_id))
            .first(conn)
            .context("failed to find content")
    }

    /// Search content
    pub fn search(conn: &SqliteConnection, params: &SearchScrape) -> anyhow::Result<Vec<Content>> {
        use crate::schema::contents::dsl;
        use diesel::prelude::*;
        let mut query = dsl::contents.into_boxed();
        if let Some(content) = params.content {
            let needle = format!("%{}%", content);
            query = query.filter(lower(dsl::searchable_content).like(needle));
        }
        query
            .load::<Content>(conn)
            .context("failed to search content")
    }
}

/// New content
#[derive(Debug, Insertable)]
#[table_name = "contents"]
pub struct NewContent<'a> {
    /// Scrape ID
    pub scrape_id: i32,
    /// Content
    pub content: &'a [u8],
    /// Searchable content
    pub searchable_content: Option<&'a str>,
}

impl<'a> NewContent<'a> {
    /// Save content to database
    pub fn save(&self, conn: &SqliteConnection) -> anyhow::Result<i32> {
        use crate::schema::contents::dsl;
        use diesel::prelude::*;

        diesel::insert_into(dsl::contents)
            .values(self)
            .execute(conn)
            .context("failed to save content")?;

        let row_id = diesel::select(last_insert_rowid).get_result::<i32>(conn)?;
        Ok(row_id)
    }
}
