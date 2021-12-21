CREATE TABLE users
(
    id                 INTEGER PRIMARY KEY NOT NULL,
    username           VARCHAR             NOT NULL,
    encrypted_password VARCHAR             NOT NULL,
    created_at         TIMESTAMP           NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE UNIQUE INDEX index_users_on_username ON users (username);

CREATE TABLE entries
(
    id              INTEGER PRIMARY KEY NOT NULL,
    user_id         INTEGER             NOT NULL,
    title           VARCHAR,
    url             VARCHAR             NOT NULL,
    hashed_url      VARCHAR             NOT NULL,
    origin_url      VARCHAR,
    is_archived     BOOLEAN             NOT NULL DEFAULT 'f',
    archived_at     TIMESTAMP,
    is_starred      BOOLEAN             NOT NULL DEFAULT 'f',
    created_at      TIMESTAMP           NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at      TIMESTAMP           NOT NULL DEFAULT CURRENT_TIMESTAMP,
    published_at    TIMESTAMP,
    published_by    TEXT,
    starred_at      TIMESTAMP,
    mime_type       VARCHAR,
    language        VARCHAR,
    reading_time    INTEGER             NOT NULL DEFAULT 0,
    domain_name     VARCHAR,
    preview_picture VARCHAR,
    http_status     INTEGER             NOT NULL,
    headless        BOOLEAN             NOT NULL DEFAULT 'f',
    FOREIGN KEY (user_id) REFERENCES users (id),
    FOREIGN KEY (published_by) REFERENCES users (id)
);
CREATE INDEX index_scrapes_on_user_id ON entries (user_id);
CREATE INDEX index_scrapes_on_published_by ON entries (published_by);
CREATE UNIQUE INDEX index_scrapes_on_url ON entries (user_id, url);

CREATE TABLE contents
(
    id                 INTEGER PRIMARY KEY NOT NULL,
    entry_id           INTEGER             NOT NULL,
    content            BLOB                NOT NULL,
    searchable_content TEXT,
    created_at         TIMESTAMP           NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (entry_id) REFERENCES entries (id)
);
CREATE INDEX index_contents_on_entry_id ON contents (entry_id);
