use chrono::NaiveDateTime;
use sqlx::FromRow;

/// Row of the `users` table.
#[derive(Debug, Clone, FromRow)]
pub struct UserRow {
    pub id: u64,
    pub guid: String,
    pub username: String,
    pub email: String,
    pub password_hash: String,
    pub avatar: Option<String>,
    pub last_logged_in: Option<NaiveDateTime>,
}

/// Row of the `sessions` table (one per user).
#[derive(Debug, Clone, FromRow)]
pub struct SessionRow {
    pub session_key: String,
    pub user_id: u64,
}

/// Row of the `password_reset_otps` table.
#[derive(Debug, Clone, FromRow)]
pub struct OtpRow {
    pub user_id: u64,
    pub code: String,
    pub expires_at: NaiveDateTime,
}

/// A pending outbox event, written transactionally with a state change.
#[derive(Debug, Clone)]
pub struct NewOutbox<'a> {
    pub event_id: &'a str,
    pub topic: &'a str,
    pub payload: &'a serde_json::Value,
}

/// Row of the `outbox` table.
#[derive(Debug, Clone, FromRow)]
pub struct OutboxRow {
    pub id: u64,
    pub event_id: String,
    pub topic: String,
    pub payload: serde_json::Value,
}
