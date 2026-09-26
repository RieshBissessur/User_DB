//! Schema-agnostic database plumbing shared by the services:
//! a connection pool, a migration runner, an injection-safe `SELECT` builder,
//! and generic row lookups that work with any `FromRow` struct.
//!
//! Knows nothing about service schemas. Schema-specific SQL lives in each
//! service's `src/db/repo.rs`. The builder is unit-tested against SQLite.

#![cfg_attr(
    not(test),
    deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)
)]

pub mod bind;
pub mod find;
pub mod pool;
pub mod query;

pub use bind::{bind_all, Bind};
pub use find::{find_all, find_by, find_by_id, find_one};
pub use pool::{connect, run_migrations};
pub use query::{Direction, QueryError, Select};
