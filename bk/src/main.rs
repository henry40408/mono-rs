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

use std::collections::HashMap;
use std::io;
use std::io::Write;

use anyhow::bail;
use comfy_table::Table;
use diesel::SqliteConnection;
use env_logger::Env;
use log::{debug, info};
use structopt::StructOpt;

use bk::entities::{Content, NewScrape, NewUser, Scrape, SearchScrape, User};
use bk::prelude::*;
use bk::{connect_database, migrate_database, Scraped, Scraper};

#[derive(Debug, StructOpt)]
#[structopt(about, author)]
enum Commands {
    /// Scrape web page with URL
    Scrape {
        #[structopt(long)]
        /// Scrape with headless Chromium?
        headless: bool,
        #[structopt(name = "URLS")]
        /// URLs to be scraped
        urls: Vec<String>,
    },
    /// List or search scrapes
    Search {
        #[structopt(short, long)]
        /// Search URL
        url: Option<String>,
        #[structopt(short, long)]
        /// Search content
        content: Option<String>,
        #[structopt(short, long)]
        /// Search title
        title: Option<String>,
    },
    /// Scrape and save to database
    Add {
        #[structopt(short, long, env = "USER_ID")]
        /// User ID
        user_id: i32,
        #[structopt(short, long)]
        /// Overwrite if entry exists
        force: bool,
        #[structopt(long)]
        /// Scrape with headless Chromium?
        headless: bool,
        #[structopt(name = "URLS")]
        /// URLs to be scraped
        urls: Vec<String>,
    },
    /// Show scraped content
    Content {
        #[structopt(short, long)]
        /// Primary key
        id: i32,
    },
    Delete {
        #[structopt(short, long)]
        /// Primary key
        id: i32,
    },
    /// Show metadata scraped
    Show {
        #[structopt(short, long)]
        /// Primary key
        id: i32,
    },
    /// Manage users
    User(UserCommand),
}

/// Manage users
#[derive(Debug, StructOpt)]
#[structopt()]
enum UserCommand {
    /// Add user
    ///
    /// Password should be set through standard input
    ///
    /// e.g. echo password | bk user add -u user
    Add {
        /// Username
        #[structopt(short, long)]
        username: String,
    },
    /// List users
    List,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().ok();
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let conn = connect_database()?;
    migrate_database(&conn)?;

    let commands = Commands::from_args();
    match commands {
        Commands::Scrape { ref urls, .. } => scrape(urls).await?,
        Commands::Search {
            url,
            content,
            title,
        } => {
            let mut params = SearchScrape {
                url: url.as_deref(),
                title: title.as_deref(),
                content: content.as_deref(),
                users: Some(HashMap::<i32, User>::new()),
                contents: Some(HashMap::<i32, Content>::new()),
            };
            search(&mut params).await?
        }
        Commands::Add {
            user_id,
            force,
            headless,
            ref urls,
        } => save_many(user_id, force, urls, headless).await?,
        Commands::Content { id } => show_content(&conn, id)?,
        Commands::Delete { id } => delete(&conn, id)?,
        Commands::Show { id } => show(&conn, id)?,
        Commands::User(u) => match u {
            UserCommand::Add { ref username } => add_user(username)?,
            UserCommand::List => list_users()?,
        },
    }
    Ok(())
}

fn add_user(username: &str) -> anyhow::Result<()> {
    let mut password = String::new();
    std::io::stdin().read_line(&mut password)?;
    password = password.trim().into();

    if password.is_empty() {
        bail!("password is required")
    } else {
        debug!("read {} byte(s) as password", password.len());
    }

    let conn = connect_database()?;
    let new_user = NewUser {
        username,
        password: &password,
    };
    let rows_affected = new_user.save(&conn)?;
    info!("{} user(s) created", rows_affected);

    Ok(())
}

fn list_users() -> anyhow::Result<()> {
    let conn = connect_database()?;
    let users = User::list(&conn)?;
    println!("{} user(s)", users.len());

    let mut table = Table::new();
    table.set_header(vec!["ID", "Username", "Created at"]);
    for user in users {
        table.add_row(vec![
            user.id.to_string(),
            user.username,
            user.created_at.rfc3339(),
        ]);
    }
    println!("{}", table);
    Ok(())
}

async fn scrape(urls: &[String]) -> anyhow::Result<()> {
    for url in urls {
        let new_doc = Scraper::from_url(url);
        let scraped = new_doc.scrape().await?;
        if let Scraped::Document(ref doc) = scraped {
            println!("{}", doc.html);
        }
        if let Scraped::Blob(ref blob) = scraped {
            eprintln!(
                "binary content, MIME type = {}, content length = {}",
                blob.mime_type.mime_type(),
                blob.content.len()
            );
        }
    }
    Ok(())
}

async fn search(params: &mut SearchScrape<'_>) -> anyhow::Result<()> {
    let conn = connect_database()?;
    let scrapes = Scrape::search(&conn, params)?;

    println!("total {}", scrapes.len());

    let mut table = Table::new();
    table.set_header(vec![
        "ID",
        "User",
        "URL",
        "Created at",
        "Title",
        "Size",
        "Traits",
    ]);
    for scrape in scrapes {
        table.add_row(vec![
            scrape.id.to_string(),
            match &params.users {
                Some(users) => match users.get(&scrape.user_id) {
                    Some(user) => user.username.to_string(),
                    None => "".to_string(),
                },
                None => "".to_string(),
            },
            scrape.url.clone(),
            scrape.created_at.rfc3339(),
            match scrape.title {
                None => "".to_string(),
                Some(ref t) => t.clone(),
            },
            match params.contents {
                None => "".to_string(),
                Some(ref contents) => match contents.get(&scrape.id) {
                    Some(content) => content.content.len().to_string(),
                    None => "".to_string(),
                },
            },
            format!("{}", scrape.traits()),
        ]);
    }
    println!("{}", table);

    Ok(())
}

async fn save_many(
    user_id: i32,
    force: bool,
    urls: &[String],
    headless: bool,
) -> anyhow::Result<()> {
    let conn = connect_database()?;

    let mut tasks = vec![];
    for url in urls {
        tasks.push(save_one(&conn, user_id, force, url, headless));
    }

    let results = futures::future::join_all(tasks).await;
    for result in results {
        let _result = result?;
    }

    Ok(())
}

async fn save_one(
    conn: &SqliteConnection,
    user_id: i32,
    force: bool,
    url: &str,
    headless: bool,
) -> anyhow::Result<()> {
    let scraper = Scraper::from_url(url)
        .with_user_id(user_id)
        .with_force(force)
        .with_headless(headless);

    let scraped = scraper.scrape().await?;

    let new_scrape = NewScrape::from(scraped);
    new_scrape.save(conn)?;

    Ok(())
}

fn show_content(conn: &SqliteConnection, id: i32) -> anyhow::Result<()> {
    let c = Content::find_by_scrape_id(conn, id)?;
    io::stdout().write_all(c.content.as_slice())?;
    io::stdout().flush()?;
    info!("{} byte(s) written", c.content.len());
    Ok(())
}

fn delete(conn: &SqliteConnection, id: i32) -> anyhow::Result<()> {
    let count = Scrape::delete(conn, id)?;
    info!("{} scrape(s) deleted", count);
    Ok(())
}

fn show(conn: &SqliteConnection, id: i32) -> anyhow::Result<()> {
    let scrape = Scrape::find(conn, id)?;
    let content = Content::find_by_scrape_id(conn, id)?;
    let mut table = Table::new();
    table.set_header(vec!["Name".to_string(), "Value".to_string()]);
    table.add_row(vec!["ID".to_string(), scrape.id.to_string()]);
    table.add_row(vec!["URL".into(), scrape.url]);
    table.add_row(vec!["Headless?".into(), scrape.headless.to_string()]);
    table.add_row(vec![
        "Title".into(),
        scrape.title.map_or("".to_string(), |t| t),
    ]);
    table.add_row(vec![
        "Content Length".into(),
        content.content.len().to_string(),
    ]);
    table.add_row(vec![
        "Searchable?".into(),
        content.searchable_content.is_some().to_string(),
    ]);
    table.add_row(vec!["Created at".into(), scrape.created_at.rfc3339()]);
    println!("{}", table);
    Ok(())
}

#[cfg(test)]
mod test {
    use diesel::connection::SimpleConnection;
    use diesel::{Connection, SqliteConnection};

    use crate::entities::NewUser;
    use crate::{connect_database, migrate_database, save_one};

    fn setup() -> anyhow::Result<SqliteConnection> {
        std::env::set_var("DATABASE_URL", "test.sqlite3");
        let conn = connect_database()?;
        conn.batch_execute("PRAGMA busy_timeout = 5000;")?;
        migrate_database(&conn)?;
        Ok(conn)
    }

    #[tokio::test]
    async fn test_save() -> anyhow::Result<()> {
        let conn = setup()?;
        conn.begin_test_transaction()?;

        let username = "user";
        let password = "password";
        let user = NewUser { username, password };
        let user_id = user.save(&conn)?;

        let url = "https://www.example.com";
        save_one(&conn, user_id, false, &url, false).await?;

        Ok(())
    }
}
