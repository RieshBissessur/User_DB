use sqlx::migrate::Migrator;
use sqlx::mysql::{MySqlPool, MySqlPoolOptions};

/// Connect to MySQL.
pub async fn connect(database_url: &str) -> Result<MySqlPool, sqlx::Error> {
    MySqlPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await
}

/// Run the caller's embedded migrations. Generic so tests can use SQLite.
pub async fn run_migrations<'a, A>(
    pool: A,
    migrator: &Migrator,
) -> Result<(), sqlx::migrate::MigrateError>
where
    A: sqlx::Acquire<'a>,
    <A::Database as sqlx::Database>::Connection: sqlx::migrate::Migrate,
{
    migrator.run(pool).await
}
