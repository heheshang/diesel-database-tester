// @generated automatically by Diesel CLI.

diesel::table! {
    todos (id) {
        id -> Int4,
        title -> Varchar,
        completed -> Nullable<Bool>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}
