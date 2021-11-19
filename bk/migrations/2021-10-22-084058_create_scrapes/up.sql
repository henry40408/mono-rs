CREATE TABLE users (
    id INTEGER PRIMARY KEY NOT NULL,
    username VARCHAR NOT NULL,
    encrypted_password VARCHAR NOT NULL ,
    created_at TIMESTAMP NOT NULL  DEFAULT  CURRENT_TIMESTAMP
);
CREATE UNIQUE INDEX index_users_on_username ON users (username);

CREATE TABLE scrapes (
    id INTEGER PRIMARY KEY NOT NULL,
    url VARCHAR NOT NULL,
    headless BOOLEAN NOT NULL DEFAULT 'f',
    content BLOB NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE UNIQUE INDEX index_scrapes_on_url ON scrapes (url);
