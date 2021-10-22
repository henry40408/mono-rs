table! {
    scrapes (id) {
        id -> Int4,
        url -> Varchar,
        headless -> Bool,
        content -> Bytea,
        created_at -> Timestamp,
    }
}
