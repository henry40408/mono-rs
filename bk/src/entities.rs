use chrono::NaiveDateTime;
use diesel::SqliteConnection;

use crate::schema::{scrapes, users};

/// User
#[derive(Debug, Queryable)]
pub struct User {
    /// Primary key
    pub id: i32,
    /// Username
    pub username: String,
    /// Encrypted password
    pub encrypted_password: String,
    /// When the user is created
    pub created_at: NaiveDateTime,
}

/// New user
#[derive(Debug)]
pub struct NewUser<'a> {
    /// Username
    pub username: &'a str,
    /// Raw password, will be encrypted before save to database
    pub password: &'a str,
}

/// New user with encrypted password
#[derive(Debug, Insertable)]
#[table_name = "users"]
pub struct NewUserWithEncryptedPassword {
    /// Username
    pub username: String,
    /// Encrypted password
    pub encrypted_password: String,
}

/// Create user
pub fn create_user(conn: &SqliteConnection, new_user: &NewUser) -> anyhow::Result<usize> {
    use crate::schema::users::dsl;
    use diesel::prelude::*;

    let encrypted_password = bcrypt::hash(&new_user.password, bcrypt::DEFAULT_COST)?;
    let with_encrypted_password = NewUserWithEncryptedPassword {
        username: new_user.username.to_string(),
        encrypted_password,
    };

    let affected_rows = diesel::insert_into(dsl::users)
        .values(with_encrypted_password)
        .execute(conn)?;
    Ok(affected_rows)
}

/// Parameters to validate user e.g. sign-in
#[derive(Debug)]
pub struct ValidateUser<'a> {
    /// Username
    pub username: &'a str,
    /// Password to be validated
    pub password: &'a str,
}

/// Validate user
pub fn validate_user(conn: &SqliteConnection, params: &ValidateUser) -> Option<User> {
    use crate::schema::users::dsl;
    use diesel::prelude::*;

    let mut query = dsl::users.into_boxed();
    query = query.filter(dsl::username.eq(&params.username));

    let users: Vec<User> = query.load::<User>(conn).ok()?;
    if let Some(user) = users.first() {
        if bcrypt::verify(&params.password, &user.encrypted_password).ok()? {
            users.into_iter().next()
        } else {
            None
        }
    } else {
        None
    }
}

/// Scrape
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
    pub created_at: NaiveDateTime,
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
        conn: &SqliteConnection,
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
    pub fn save(&self, conn: &SqliteConnection) -> diesel::result::QueryResult<()> {
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
    use diesel::connection::SimpleConnection;
    use diesel::result::Error;
    use diesel::{Connection, SqliteConnection};

    use crate::embedded_migrations;
    use crate::entities::{
        create_user, validate_user, NewScrape, NewUser, Scrape, SearchScrape, ValidateUser,
    };
    use crate::{connect_database, Scraper};

    fn setup() -> anyhow::Result<SqliteConnection> {
        std::env::set_var("DATABASE_URL", "test.sqlite3");
        let conn = connect_database()?;
        conn.batch_execute("PRAGMA busy_timeout = 5000;")?;
        embedded_migrations::run(&conn)?;
        Ok(conn)
    }

    #[tokio::test]
    async fn test_validate_user() -> anyhow::Result<()> {
        let conn = setup()?;
        conn.test_transaction::<_, Error, _>(|| {
            let username = "user";
            let password = "password";

            let new_user = NewUser { username, password };

            let res = create_user(&conn, &new_user);
            let rows_effected = res.unwrap();
            assert_eq!(1, rows_effected);

            let params = ValidateUser { username, password };

            let res = validate_user(&conn, &params);
            let user = res.unwrap();
            assert_eq!(user.username, username);
            assert_ne!(user.encrypted_password, password);

            Ok(())
        });
        Ok(())
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
