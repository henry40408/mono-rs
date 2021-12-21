table! {
    contents (id) {
        id -> Integer,
        entry_id -> Integer,
        content -> Binary,
        searchable_content -> Nullable<Text>,
        created_at -> Timestamp,
    }
}

table! {
    entries (id) {
        id -> Integer,
        user_id -> Integer,
        title -> Nullable<Text>,
        url -> Text,
        hashed_url -> Text,
        origin_url -> Nullable<Text>,
        is_archived -> Bool,
        archived_at -> Nullable<Timestamp>,
        is_starred -> Bool,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        published_at -> Nullable<Timestamp>,
        published_by -> Nullable<Text>,
        starred_at -> Nullable<Timestamp>,
        mime_type -> Nullable<Text>,
        language -> Nullable<Text>,
        reading_time -> Integer,
        domain_name -> Nullable<Text>,
        preview_picture -> Nullable<Text>,
        http_status -> Integer,
        headless -> Bool,
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

joinable!(contents -> entries (entry_id));

allow_tables_to_appear_in_same_query!(contents, entries, users,);
