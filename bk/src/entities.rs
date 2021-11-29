use anyhow::{bail, Context};
use chrono::NaiveDateTime;
use diesel::sql_types::{Integer, Nullable, Text};
use diesel::SqliteConnection;
use std::collections::HashMap;
use std::fmt::{Display, Formatter};

use crate::schema::{contents, scrapes, users};

sql_function! {
    /// LOWER(t)
    fn lower(a: Nullable<Text>) -> Nullable<Text>;
}

no_arg_sql_function!(
    last_insert_rowid,
    Integer,
    "Represents the SQL last_insert_row() function"
);

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
    pub fn save(&self, conn: &SqliteConnection) -> anyhow::Result<i32> {
        use crate::schema::users::dsl;
        use diesel::prelude::*;

        let encrypted_password = bcrypt::hash(&self.password, bcrypt::DEFAULT_COST)?;
        let with_encrypted_password = NewUserWithEncryptedPassword {
            username: self.username,
            encrypted_password: &encrypted_password,
        };

        diesel::insert_into(dsl::users)
            .values(with_encrypted_password)
            .execute(conn)?;
        let row_id = diesel::select(last_insert_rowid).get_result::<i32>(conn)?;
        Ok(row_id)
    }
}

/// New user with encrypted password
#[derive(Debug, Insertable)]
#[table_name = "users"]
pub struct NewUserWithEncryptedPassword<'a> {
    /// Username
    pub username: &'a str,
    /// Encrypted password
    pub encrypted_password: &'a str,
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

#[cfg(test)]
mod test {
    use diesel::connection::SimpleConnection;
    use diesel::{Connection, SqliteConnection};
    use std::collections::HashMap;

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
        conn.begin_test_transaction()?;

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
    }

    #[tokio::test]
    async fn test_search() -> anyhow::Result<()> {
        let conn = setup()?;
        let mut params = SearchScrape::default();
        let scrapes = Scrape::search(&conn, &mut params)?;
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

        let new_user = NewUser { username, password };
        let user_id = new_user.save(&conn).unwrap();

        let scraper = Scraper::from_url("https://www.example.com").with_user_id(user_id);

        let scraped = scraper.scrape().await?;

        let new_scrape = NewScrape::from(scraped);
        let res = new_scrape.save(&conn);
        let rows_affected = res.unwrap();
        assert_eq!(1, rows_affected);

        let mut params = SearchScrape::default();
        params.url = Some("example".into());
        params.users = Some(HashMap::<i32, User>::new());

        let res = Scrape::search(&conn, &mut params);
        assert_eq!(1, params.users.unwrap().len());

        let scrapes = res.unwrap();
        assert_eq!(1, scrapes.len());

        let scrape = scrapes.first().unwrap();
        assert_eq!(Some("Example Domain"), scrape.title.as_deref());

        Ok(())
    }
}
