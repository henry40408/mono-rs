use diesel::sql_types::{Integer, Nullable, Text};

pub use content::{Content, NewContent};
pub use entry::{Entry, NewPartialEntry, SearchScrape};
pub use user::{Authentication, NewUser, User};

sql_function! {
    /// LOWER(t)
    fn lower(a: Nullable<Text>) -> Nullable<Text>;
}

no_arg_sql_function!(
    last_insert_rowid,
    Integer,
    "Represents the SQL last_insert_row() function"
);

/// Content
pub mod content;

/// Scrape
pub mod entry;

/// User
pub mod user;

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use diesel::connection::SimpleConnection;
    use diesel::{Connection, SqliteConnection};

    use crate::embedded_migrations;
    use crate::entities::{Authentication, Entry, NewPartialEntry, NewUser, SearchScrape, User};
    use crate::{connect_database, Scraper};

    fn setup() -> anyhow::Result<SqliteConnection> {
        std::env::set_var("DATABASE_URL", "test.sqlite3");
        let conn = connect_database()?;
        conn.batch_execute("PRAGMA busy_timeout = 5000;")?;
        embedded_migrations::run(&conn)?;
        Ok(conn)
    }

    #[tokio::test]
    async fn test_authentication_find() -> anyhow::Result<()> {
        let conn = setup()?;
        conn.begin_test_transaction()?;

        let username = "user";
        let password = "password";
        let email = "user@example.com";
        let name = "User";

        let new_user = NewUser {
            username,
            password,
            email,
            name,
        };
        let res = new_user.save(&conn);
        let rows_affected = res.unwrap();
        assert_eq!(1, rows_affected);

        let auth = Authentication { username, password };
        let res = auth.authenticate(&conn);
        let user = res.unwrap();
        assert_eq!(user.username, username);
        assert_ne!(user.encrypted_password, password);

        let res = User::single(&conn);
        let user = res.unwrap();
        assert_eq!(user.username, username);

        let res = User::find(&conn, user.id);
        let found = res.unwrap();
        assert_eq!(found.id, user.id);

        Ok(())
    }

    #[tokio::test]
    async fn test_search() -> anyhow::Result<()> {
        let conn = setup()?;
        let mut params = SearchScrape::default();
        let scrapes = Entry::search(&conn, &mut params)?;
        assert!(params.users.is_none());
        assert_eq!(0, scrapes.len());
        Ok(())
    }

    #[tokio::test]
    async fn test_save_and_search() -> anyhow::Result<()> {
        let conn = setup()?;
        conn.begin_test_transaction()?;

        let username = "user";
        let password = "password";
        let email = "user@example.com";
        let name = "User";

        let new_user = NewUser {
            username,
            password,
            email,
            name,
        };
        let user_id = new_user.save(&conn).unwrap();

        let scraper = Scraper::from_url("https://www.example.com").with_user_id(user_id);

        let scraped = scraper.scrape().await?;

        let new_scrape = NewPartialEntry::from(scraped);
        let res = new_scrape.save(&conn);
        let rows_affected = res.unwrap();
        assert_eq!(1, rows_affected);

        let mut params = SearchScrape::default();
        params.url = Some("example".into());
        params.users = Some(HashMap::<i32, User>::new());

        let res = Entry::search(&conn, &mut params);
        assert_eq!(1, params.users.unwrap().len());

        let scrapes = res.unwrap();
        assert_eq!(1, scrapes.len());

        let scrape = scrapes.first().unwrap();
        assert_eq!(Some("Example Domain"), scrape.title.as_deref());

        Ok(())
    }
}
