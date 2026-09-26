use sqlx::mysql::{MySql, MySqlArguments};
use sqlx::query::{Query, QueryAs, QueryScalar};

/// A value bound into a query, in placeholder order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Bind {
    Text(String),
    Int(i64),
    UInt(u64),
    Bool(bool),
    Null,
}

impl From<&str> for Bind {
    fn from(value: &str) -> Self {
        Self::Text(value.to_string())
    }
}

impl From<String> for Bind {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl From<i64> for Bind {
    fn from(value: i64) -> Self {
        Self::Int(value)
    }
}

impl From<u64> for Bind {
    fn from(value: u64) -> Self {
        Self::UInt(value)
    }
}

impl From<bool> for Bind {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

/// Anything that accepts binds in order: the sqlx query types we use.
pub trait BindQuery<'q, DB>: Sized {
    fn bind_value(self, bind: Bind) -> Self;
}

impl<'q> BindQuery<'q, MySql> for Query<'q, MySql, MySqlArguments> {
    fn bind_value(self, bind: Bind) -> Self {
        match bind {
            Bind::Text(value) => self.bind(value),
            Bind::Int(value) => self.bind(value),
            Bind::UInt(value) => self.bind(value),
            Bind::Bool(value) => self.bind(value),
            Bind::Null => self.bind(Option::<String>::None),
        }
    }
}

impl<'q, O> BindQuery<'q, MySql> for QueryAs<'q, MySql, O, MySqlArguments> {
    fn bind_value(self, bind: Bind) -> Self {
        match bind {
            Bind::Text(value) => self.bind(value),
            Bind::Int(value) => self.bind(value),
            Bind::UInt(value) => self.bind(value),
            Bind::Bool(value) => self.bind(value),
            Bind::Null => self.bind(Option::<String>::None),
        }
    }
}

impl<'q, O> BindQuery<'q, MySql> for QueryScalar<'q, MySql, O, MySqlArguments> {
    fn bind_value(self, bind: Bind) -> Self {
        match bind {
            Bind::Text(value) => self.bind(value),
            Bind::Int(value) => self.bind(value),
            Bind::UInt(value) => self.bind(value),
            Bind::Bool(value) => self.bind(value),
            Bind::Null => self.bind(Option::<String>::None),
        }
    }
}

/// Bind every value onto a query, in order.
pub fn bind_all<'q, Q>(query: Q, binds: impl IntoIterator<Item = Bind>) -> Q
where
    Q: BindQuery<'q, MySql>,
{
    binds
        .into_iter()
        .fold(query, |query, bind| query.bind_value(bind))
}
