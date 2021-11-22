use anyhow::{bail, Context};
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

impl User {
    /// List users
    pub fn list(conn: &SqliteConnection) -> anyhow::Result<Vec<User>> {
        use crate::schema::users::dsl;
        use diesel::prelude::*;

        let query = dsl::users.into_boxed();
        let users: Vec<User> = query.load::<User>(conn)?;
        Ok(users)
    }

    /// Find user by ID
    pub fn find(conn: &SqliteConnection, id: i32) -> anyhow::Result<User> {
        use crate::schema::users::dsl;
        use diesel::prelude::*;
        dsl::users
            .find(id)
            .first(conn)
            .context("failed to find user by ID")
    }

    /// Single user
    pub fn single(conn: &SqliteConnection) -> anyhow::Result<User> {
        use crate::schema::users::dsl;
        use diesel::dsl::count;
        use diesel::prelude::*;

        let res = dsl::users.select(count(dsl::id)).first(conn);
        if Ok(1) != res {
            match res {
                Ok(c) => bail!("{} user(s) found", c),
                Err(_e) => bail!("more than one user(s) found"),
            }
        }

        let query = dsl::users.into_boxed();
        let user: User = query.first::<User>(conn)?;
        Ok(user)
    }
}

/// New user
#[derive(Debug)]
pub struct NewUser<'a> {
    /// Username
    pub username: &'a str,
    /// Raw password, will be encrypted before save to database
    pub password: &'a str,
}

impl<'a> NewUser<'a> {
    /// Create user
    pub fn save(&self, conn: &SqliteConnection) -> anyhow::Result<usize> {
        use crate::schema::users::dsl;
        use diesel::prelude::*;

        let encrypted_password = bcrypt::hash(&self.password, bcrypt::DEFAULT_COST)?;
        let with_encrypted_password = NewUserWithEncryptedPassword {
            username: self.username.to_string(),
            encrypted_password,
        };

        let affected_rows = diesel::insert_into(dsl::users)
            .values(with_encrypted_password)
            .execute(conn)?;
        Ok(affected_rows)
    }
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

/// User authentication
#[derive(Debug)]
pub struct Authentication<'a> {
    /// Username
    pub username: &'a str,
    /// Password
    pub password: &'a str,
}

impl<'a> Authentication<'a> {
    /// Validate user
    pub fn authenticate(&self, conn: &SqliteConnection) -> Option<User> {
        use crate::schema::users::dsl;
        use diesel::prelude::*;

        let mut query = dsl::users.into_boxed();
        query = query.filter(dsl::username.eq(self.username));

        let res = query.first::<User>(conn);
        if let Ok(user) = res {
            if bcrypt::verify(self.password, &user.encrypted_password).ok()? {
                Some(user)
            } else {
                None
            }
        } else {
            None
        }
    }
}

/// Scrape
#[derive(Debug, Queryable, Insertable)]
pub struct Scrape {
    /// Primary key
    pub id: i32,
    /// User ID
    pub user_id: i32,
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
    /// Find scrape with ID
    pub fn find(conn:&SqliteConnection, id:i32) ->anyhow::Result<Scrape> {
        use crate::schema::scrapes::dsl;
        use diesel::prelude::*;
        dsl::scrapes.find(id).first(conn).context("cannot find scrape with ID")
    }

    /// Search scrapes with parameters
    pub fn search(conn: &SqliteConnection, params: &SearchScrape) -> anyhow::Result<Vec<Scrape>> {
        use crate::schema::scrapes::dsl;
        use diesel::prelude::*;

        let mut query = dsl::scrapes.into_boxed();

        let url = params.url.as_ref().map(|u| format!("%{}%", u));
        if let Some(url) = url {
            query = query.filter(dsl::url.like(url));
        }

        query
            .load::<Scrape>(conn)
            .context("failed to search scrapes")
    }
}

/// New scrape
#[derive(Debug)]
pub struct NewScrape {
    /// URL scraped
    pub url: String,
    /// Optional user ID
    pub user_id: Option<i32>,
    /// Scrape with headless Chromium
    pub headless: bool,
    /// Actual content from URL
    pub content: Vec<u8>,
}

impl NewScrape {
    /// Save scrape
    pub fn save(&self, conn: &SqliteConnection) -> anyhow::Result<usize> {
        let res = match self.user_id {
            None => User::single(conn),
            Some(id) => User::find(conn, id),
        };
        let user = res?;
        let new_scrape = StrictNewScrape {
            url: self.url.to_string(),
            user_id: user.id,
            headless: self.headless,
            content: self.content.clone(),
        };
        new_scrape.save(conn)
    }
}

/// New scrape to database
#[derive(Debug, Insertable)]
#[table_name = "scrapes"]
pub struct StrictNewScrape {
    /// URL scraped
    pub url: String,
    /// User ID
    pub user_id: i32,
    /// Scrape with headless Chromium
    pub headless: bool,
    /// Actual content from URL
    pub content: Vec<u8>,
}

impl StrictNewScrape {
    fn save(&self, conn: &SqliteConnection) -> anyhow::Result<usize> {
        use crate::schema::scrapes::dsl;
        use diesel::prelude::*;

        diesel::insert_into(dsl::scrapes)
            .values(self)
            .execute(conn)
            .context("failed to save scrape")
    }
}

#[cfg(test)]
mod test {
    use diesel::connection::SimpleConnection;
    use diesel::result::Error;
    use diesel::{Connection, SqliteConnection};

    use crate::embedded_migrations;
    use crate::entities::{Authentication, NewScrape, NewUser, Scrape, SearchScrape, User};
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
        conn.test_transaction::<_, Error, _>(|| {
            let username = "user";
            let password = "password";

            let new_user = NewUser { username, password };
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
        conn.test_transaction::<_, Error, _>(|| {
            let username = "user";
            let password = "password";

            let new_user = NewUser { username, password };
            new_user.save(&conn).unwrap();

            let new_scrape = NewScrape::from(scraped);
            let res = new_scrape.save(&conn);
            let rows_affected = res.unwrap();
            assert_eq!(1, rows_affected);

            let mut params = SearchScrape::default();
            params.url = Some("example".into());

            let res = Scrape::search(&conn, &params);
            let scrapes = res.unwrap();
            assert_eq!(1, scrapes.len());

            Ok(())
        });

        Ok(())
    }
}
