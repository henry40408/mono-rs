use anyhow::{bail, Context};
use chrono::NaiveDateTime;
use diesel::SqliteConnection;

use crate::entities::last_insert_rowid;
use crate::schema::users;

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
