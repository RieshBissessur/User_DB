use sqlx::migrate::Migrator;

/// Embedded migrations, applied on startup and by `#[sqlx::test]`.
pub static MIGRATOR: Migrator = sqlx::migrate!("./migrations");
