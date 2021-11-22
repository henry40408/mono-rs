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

use anyhow::bail;
use bk::entities::{NewScrape, NewUser, Scrape, SearchScrape, User};
use bk::{connect_database, migrate_database, Scraped, Scraper};
use chrono::Utc;
use comfy_table::Table;
use diesel::SqliteConnection;
use structopt::StructOpt;

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
    /// Search URL in database
    Search {
        #[structopt(short, long)]
        /// URL to be searched
        url: Option<String>,
    },
    /// Scrape and save to database
    Save {
        #[structopt(long)]
        /// Scrape with headless Chromium?
        headless: bool,
        #[structopt(name = "URLS")]
        /// URLs to be scraped
        urls: Vec<String>,
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

    let conn = connect_database()?;
    migrate_database(&conn)?;

    let commands = Commands::from_args();
    match commands {
        Commands::Scrape { ref urls, .. } => scrape(urls).await?,
        Commands::Search { ref url } => search(url).await?,
        Commands::Save { ref urls, headless } => save_many(urls, headless).await?,
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
    password = password.trim().to_string();

    if password.is_empty() {
        bail!("password is required")
    } else {
        eprintln!("read {} byte(s) as password", password.len());
    }

    let conn = connect_database()?;
    let new_user = NewUser {
        username,
        password: &password,
    };
    let rows_affected = new_user.save(&conn)?;
    println!("{} user(s) created", rows_affected);

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
            user.created_at.to_string(),
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

async fn search(url: &Option<String>) -> anyhow::Result<()> {
    let params = SearchScrape {
        url: url.to_owned(),
    };

    let conn = connect_database()?;
    let scrapes = Scrape::search(&conn, &params)?;

    println!("total {}", scrapes.len());

    let mut table = Table::new();
    table.set_header(vec!["ID", "URL", "Headless?", "Created at", "Size"]);
    for scrape in scrapes {
        let created_at = chrono::DateTime::<Utc>::from_utc(scrape.created_at, Utc);
        table.add_row(vec![
            scrape.id.to_string(),
            scrape.url,
            scrape.headless.to_string(),
            created_at.to_rfc3339(),
            scrape.content.len().to_string(),
        ]);
    }
    println!("{}", table);

    Ok(())
}

async fn save_many(urls: &[String], headless: bool) -> anyhow::Result<()> {
    let conn = connect_database()?;

    let mut tasks = vec![];
    for url in urls {
        tasks.push(save_one(&conn, url, headless));
    }

    let results = futures::future::join_all(tasks).await;
    for result in results {
        let _result = result?;
    }

    Ok(())
}

async fn save_one(conn: &SqliteConnection, url: &str, headless: bool) -> anyhow::Result<()> {
    let mut scraper = Scraper::from_url(url);
    scraper.headless = headless;

    let scraped = scraper.scrape().await?;

    let new_scrape = NewScrape::from(scraped);
    new_scrape.save(conn)?;

    Ok(())
}

#[cfg(test)]
mod test {
    use crate::save_one;
    use bk::entities::NewUser;
    use bk::{connect_database, migrate_database};
    use diesel::connection::SimpleConnection;
    use diesel::{Connection, SqliteConnection};

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
        let new_user = NewUser { username, password };

        new_user.save(&conn).unwrap();

        let url = "https://www.example.com";
        save_one(&conn, &url, false).await?;

        Ok(())
    }
}
