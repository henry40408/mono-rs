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

use std::env;

use diesel::{Connection, SqliteConnection};

pub use crate::scraper::{Scraped, Scraper};

#[allow(missing_docs)]
pub mod schema;

/// Database models
pub mod entities;

/// Prelude
pub mod prelude;

/// Scraper, the library uses failure so we isolate it
pub mod scraper;

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
