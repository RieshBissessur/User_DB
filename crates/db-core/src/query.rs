use crate::bind::Bind;

/// Sort direction for [`Select::order_by`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Asc,
    Desc,
}

impl Direction {
    fn as_sql(self) -> &'static str {
        match self {
            Direction::Asc => "ASC",
            Direction::Desc => "DESC",
        }
    }
}

/// Rejected builder input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryError {
    /// Identifier is not a plain `[A-Za-z_][A-Za-z0-9_]*`.
    InvalidIdentifier(String),
    /// `WHERE IN` with no values.
    EmptyWhereIn(String),
}

impl std::fmt::Display for QueryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QueryError::InvalidIdentifier(ident) => {
                write!(f, "invalid SQL identifier: {ident:?}")
            }
            QueryError::EmptyWhereIn(column) => {
                write!(f, "WHERE IN on {column:?} has no values")
            }
        }
    }
}

impl std::error::Error for QueryError {}

/// Lets `build()?` work in functions returning `sqlx::Error`.
impl From<QueryError> for sqlx::Error {
    fn from(error: QueryError) -> Self {
        sqlx::Error::Configuration(Box::new(error))
    }
}

/// Injection-safe `SELECT` builder: values become binds, identifiers are
/// validated. Simple selects only — joins/writes stay as plain SQL.
#[derive(Debug, Clone)]
pub struct Select {
    table: String,
    columns: Vec<String>,
    conditions: Vec<String>,
    binds: Vec<Bind>,
    order: Option<(String, Direction)>,
    limit: Option<u32>,
    offset: Option<u32>,
    errors: Vec<QueryError>,
}

impl Select {
    /// Start a `SELECT`; identifiers are validated at `build()`.
    pub fn new(table: &str, columns: &[&str]) -> Self {
        let mut select = Self {
            table: table.to_string(),
            columns: columns.iter().map(|c| c.to_string()).collect(),
            conditions: Vec::new(),
            binds: Vec::new(),
            order: None,
            limit: None,
            offset: None,
            errors: Vec::new(),
        };
        select.check_identifier(table);
        for column in columns {
            select.check_identifier(column);
        }
        select
    }

    /// `WHERE column = ?`
    pub fn where_eq(mut self, column: &str, value: impl Into<Bind>) -> Self {
        self.push_condition("AND", column, "=", value.into());
        self
    }

    /// `OR column = ?` (first condition still becomes the `WHERE` clause).
    pub fn or_where_eq(mut self, column: &str, value: impl Into<Bind>) -> Self {
        self.push_condition("OR", column, "=", value.into());
        self
    }

    /// `AND column < ?`
    pub fn where_lt(mut self, column: &str, value: impl Into<Bind>) -> Self {
        self.push_condition("AND", column, "<", value.into());
        self
    }

    /// `WHERE column IS NULL`
    pub fn where_is_null(mut self, column: &str) -> Self {
        self.check_identifier(column);
        let prefix = self.joiner();
        self.conditions.push(format!("{prefix} {column} IS NULL"));
        self
    }

    /// `AND column IN (?, ?, ...)` — one bind per value.
    pub fn where_in<I, V>(mut self, column: &str, values: I) -> Self
    where
        I: IntoIterator<Item = V>,
        V: Into<Bind>,
    {
        self.check_identifier(column);
        let values: Vec<Bind> = values.into_iter().map(Into::into).collect();
        if values.is_empty() {
            self.errors
                .push(QueryError::EmptyWhereIn(column.to_string()));
            return self;
        }
        let placeholders = vec!["?"; values.len()].join(", ");
        let prefix = self.joiner();
        self.conditions
            .push(format!("{prefix} {column} IN ({placeholders})"));
        self.binds.extend(values);
        self
    }

    /// `ORDER BY column ASC|DESC`
    pub fn order_by(mut self, column: &str, direction: Direction) -> Self {
        self.check_identifier(column);
        self.order = Some((column.to_string(), direction));
        self
    }

    /// `LIMIT n`
    pub fn limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    /// `OFFSET n` (only rendered when set).
    pub fn offset(mut self, offset: u32) -> Self {
        self.offset = Some(offset);
        self
    }

    /// Finish the query: SQL with `?` placeholders plus the binds, in order.
    /// Returns the first validation error, if any.
    pub fn build(self) -> Result<(String, Vec<Bind>), QueryError> {
        if let Some(error) = self.errors.into_iter().next() {
            return Err(error);
        }

        let mut sql = format!("SELECT {} FROM {}", self.columns.join(", "), self.table);
        for condition in &self.conditions {
            sql.push(' ');
            sql.push_str(condition);
        }
        if let Some((column, direction)) = &self.order {
            sql.push_str(&format!(" ORDER BY {column} {}", direction.as_sql()));
        }
        if let Some(limit) = self.limit {
            sql.push_str(&format!(" LIMIT {limit}"));
        }
        if let Some(offset) = self.offset {
            sql.push_str(&format!(" OFFSET {offset}"));
        }
        Ok((sql, self.binds))
    }

    fn push_condition(&mut self, joiner: &str, column: &str, operator: &str, bind: Bind) {
        self.check_identifier(column);
        let prefix = if self.conditions.is_empty() {
            "WHERE"
        } else {
            joiner
        };
        self.conditions
            .push(format!("{prefix} {column} {operator} ?"));
        self.binds.push(bind);
    }

    fn joiner(&self) -> &'static str {
        if self.conditions.is_empty() {
            "WHERE"
        } else {
            "AND"
        }
    }

    fn check_identifier(&mut self, identifier: &str) {
        let valid = !identifier.is_empty()
            && identifier.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
            && identifier
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_');
        if !valid {
            self.errors
                .push(QueryError::InvalidIdentifier(identifier.to_string()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const COLS: &[&str] = &["id", "username", "email"];

    #[test]
    fn builds_a_plain_select() {
        let (sql, binds) = Select::new("users", COLS).build().unwrap();
        assert_eq!(sql, "SELECT id, username, email FROM users");
        assert!(binds.is_empty());
    }

    #[test]
    fn where_eq_binds_the_value() {
        let (sql, binds) = Select::new("users", COLS)
            .where_eq("username", "alice")
            .limit(1)
            .build()
            .unwrap();
        assert_eq!(
            sql,
            "SELECT id, username, email FROM users WHERE username = ? LIMIT 1"
        );
        assert_eq!(binds, vec![Bind::Text("alice".into())]);
    }

    #[test]
    fn or_where_eq_joins_with_or() {
        let (sql, binds) = Select::new("users", COLS)
            .where_eq("username", "alice")
            .or_where_eq("email", "alice@example.com")
            .build()
            .unwrap();
        assert_eq!(
            sql,
            "SELECT id, username, email FROM users WHERE username = ? OR email = ?"
        );
        assert_eq!(
            binds,
            vec![
                Bind::Text("alice".into()),
                Bind::Text("alice@example.com".into())
            ]
        );
    }

    #[test]
    fn where_in_expands_one_bind_per_value() {
        let (sql, binds) = Select::new("users", COLS)
            .where_in("id", [1u64, 2, 3])
            .order_by("id", Direction::Desc)
            .build()
            .unwrap();
        assert_eq!(
            sql,
            "SELECT id, username, email FROM users WHERE id IN (?, ?, ?) ORDER BY id DESC"
        );
        assert_eq!(binds, vec![Bind::UInt(1), Bind::UInt(2), Bind::UInt(3)]);
    }

    #[test]
    fn where_is_null_and_offset_render() {
        let (sql, binds) = Select::new("outbox", &["id"])
            .where_is_null("published_at")
            .order_by("id", Direction::Asc)
            .limit(100)
            .offset(50)
            .build()
            .unwrap();
        assert_eq!(
            sql,
            "SELECT id FROM outbox WHERE published_at IS NULL ORDER BY id ASC LIMIT 100 OFFSET 50"
        );
        assert!(binds.is_empty());
    }

    #[test]
    fn rejects_injection_shaped_identifiers() {
        for bad in [
            "users; DROP TABLE users",
            "name='x",
            "a b",
            "a.b",
            "",
            "1abc",
        ] {
            let error = Select::new(bad, COLS).build().unwrap_err();
            assert_eq!(error, QueryError::InvalidIdentifier(bad.to_string()));
        }
        let error = Select::new("users", COLS)
            .where_eq("name' OR '1'='1", "x")
            .build()
            .unwrap_err();
        assert!(matches!(error, QueryError::InvalidIdentifier(_)));
    }

    #[test]
    fn empty_where_in_is_an_error() {
        let error = Select::new("users", COLS)
            .where_in("id", Vec::<u64>::new())
            .build()
            .unwrap_err();
        assert_eq!(error, QueryError::EmptyWhereIn("id".to_string()));
    }

    #[test]
    fn limit_without_offset_renders_only_limit() {
        let (sql, _) = Select::new("users", COLS).limit(5).build().unwrap();
        assert!(sql.ends_with(" LIMIT 5"));
        assert!(!sql.contains("OFFSET"));
    }

    // -- round-trip against in-memory SQLite (no MySQL, no service schema) --

    #[tokio::test]
    async fn generated_sql_runs_against_sqlite() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .connect(":memory:")
            .await
            .unwrap();
        sqlx::query("CREATE TABLE users (id INTEGER PRIMARY KEY, username TEXT, email TEXT)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO users (username, email) VALUES ('alice', 'a@x'), ('bob', 'b@x')")
            .execute(&pool)
            .await
            .unwrap();

        let (sql, binds) = Select::new("users", &["id", "username", "email"])
            .where_eq("username", "bob")
            .limit(1)
            .build()
            .unwrap();

        let mut query = sqlx::query_as::<_, (i64, String, String)>(&sql);
        for bind in binds {
            query = match bind {
                Bind::Text(value) => query.bind(value),
                Bind::Int(value) => query.bind(value),
                Bind::UInt(value) => query.bind(i64::try_from(value).unwrap()),
                Bind::Bool(value) => query.bind(value),
                Bind::Null => query.bind(Option::<String>::None),
            };
        }
        let (id, username, _) = query.fetch_one(&pool).await.unwrap();
        assert_eq!(id, 2);
        assert_eq!(username, "bob");
    }

    #[tokio::test]
    async fn run_migrations_applies_a_migrator() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .connect(":memory:")
            .await
            .unwrap();

        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("0001_test.sql"),
            "CREATE TABLE marker (id INTEGER PRIMARY KEY);",
        )
        .unwrap();
        let migrator = sqlx::migrate::Migrator::new(dir.path()).await.unwrap();

        crate::run_migrations(&pool, &migrator).await.unwrap();
        let applied: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(applied, 1);
    }
}
