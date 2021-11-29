table! {
    contents (id) {
        id -> Integer,
        scrape_id -> Integer,
        content -> Binary,
        searchable_content -> Nullable<Text>,
        created_at -> Timestamp,
    }
}

table! {
    scrapes (id) {
        id -> Integer,
        user_id -> Integer,
        url -> Text,
        headless -> Bool,
        searchable -> Bool,
        title -> Nullable<Text>,
        created_at -> Timestamp,
    }
}

table! {
    users (id) {
        id -> Integer,
        username -> Text,
        encrypted_password -> Text,
        created_at -> Timestamp,
    }
}

joinable!(contents -> scrapes (scrape_id));
joinable!(scrapes -> users (user_id));

allow_tables_to_appear_in_same_query!(contents, scrapes, users,);
