CREATE TABLE users
(
    id                 INTEGER PRIMARY KEY NOT NULL,
    username           VARCHAR             NOT NULL,
    encrypted_password VARCHAR             NOT NULL,
    created_at         TIMESTAMP           NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE UNIQUE INDEX index_users_on_username ON users (username);

CREATE TABLE scrapes
(
    id         INTEGER PRIMARY KEY NOT NULL,
    user_id    INTEGER             NOT NULL,
    url        VARCHAR             NOT NULL,
    headless   BOOLEAN             NOT NULL DEFAULT 'f',
    searchable BOOLEAN             NOT NULL DEFAULT 'f',
    title      VARCHAR,
    created_at TIMESTAMP           NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users (id)
);
CREATE INDEX index_scrapes_on_user_id ON scrapes (user_id);
CREATE UNIQUE INDEX index_scrapes_on_url ON scrapes (user_id, url);

CREATE TABLE contents
(
    id                 INTEGER PRIMARY KEY NOT NULL,
    scrape_id          INTEGER             NOT NULL,
    content            BLOB                NOT NULL,
    searchable_content TEXT,
    created_at         TIMESTAMP           NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (scrape_id) REFERENCES scrapes (id)
);
CREATE INDEX index_contents_on_scrape_id ON contents (scrape_id);
