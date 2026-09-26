//! Generic row lookups. Every function returns *any* struct that implements
//! `FromRow` for MySQL — the caller decides the shape, this module decides the
//! SQL. Combined with [`Select`](crate::Select) (the typed, injection-safe way
//! to pass parameters), this is the shared default for shared repos.
//!
//! Helpers are MySQL-typed (the services run on MySQL); behaviour is covered
//! by the service integration tests.

use sqlx::mysql::MySqlRow;
use sqlx::{FromRow, MySqlPool};

use crate::bind::{bind_all, Bind};
use crate::query::Select;

/// One row by primary key (`id`).
pub async fn find_by_id<O>(
    pool: &MySqlPool,
    table: &str,
    columns: &[&str],
    id: u64,
) -> Result<Option<O>, sqlx::Error>
where
    O: for<'r> FromRow<'r, MySqlRow> + Send + Unpin,
{
    find_by(pool, table, columns, "id", id).await
}

/// One row matching a single column (`provider_id`, `username`, ...).
pub async fn find_by<O>(
    pool: &MySqlPool,
    table: &str,
    columns: &[&str],
    column: &str,
    value: impl Into<Bind>,
) -> Result<Option<O>, sqlx::Error>
where
    O: for<'r> FromRow<'r, MySqlRow> + Send + Unpin,
{
    let select = Select::new(table, columns).where_eq(column, value).limit(1);
    find_one(pool, select).await
}

/// One row for a fully built (parameterised) `Select`.
pub async fn find_one<O>(pool: &MySqlPool, select: Select) -> Result<Option<O>, sqlx::Error>
where
    O: for<'r> FromRow<'r, MySqlRow> + Send + Unpin,
{
    let (sql, binds) = select.build()?;
    let query = bind_all(sqlx::query_as::<_, O>(&sql), binds);
    query.fetch_optional(pool).await
}

/// Every row for a built `Select` (add `.limit(..)`/`.order_by(..)` first).
pub async fn find_all<O>(pool: &MySqlPool, select: Select) -> Result<Vec<O>, sqlx::Error>
where
    O: for<'r> FromRow<'r, MySqlRow> + Send + Unpin,
{
    let (sql, binds) = select.build()?;
    let query = bind_all(sqlx::query_as::<_, O>(&sql), binds);
    query.fetch_all(pool).await
}
