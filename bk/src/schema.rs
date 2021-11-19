table! {
    scrapes (id) {
        id -> Integer,
        url -> Text,
        headless -> Bool,
        content -> Binary,
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

allow_tables_to_appear_in_same_query!(
    scrapes,
    users,
);
