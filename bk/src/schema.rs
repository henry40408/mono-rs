table! {
    scrapes (id) {
        id -> Int4,
        url -> Varchar,
        headless -> Bool,
        content -> Bytea,
        created_at -> Timestamp,
    }
}

table! {
    users (id) {
        id -> Int4,
        username -> Varchar,
        encrypted_password -> Varchar,
        created_at -> Timestamp,
    }
}

allow_tables_to_appear_in_same_query!(scrapes, users,);
