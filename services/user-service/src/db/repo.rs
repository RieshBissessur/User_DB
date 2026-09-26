use chrono::NaiveDateTime;
use sqlx::mysql::MySqlPool;

use db_core::{find_all, find_by, find_one, Select};

use super::models::{NewOutbox, OtpRow, OutboxRow, SessionRow, UserRow};

const USER_COLUMNS: &[&str] = &[
    "id",
    "guid",
    "username",
    "email",
    "password_hash",
    "avatar",
    "last_logged_in",
];

// --- users -----------------------------------------------------------------

pub async fn find_user_by_username(
    pool: &MySqlPool,
    username: &str,
) -> Result<Option<UserRow>, sqlx::Error> {
    find_by(pool, "users", USER_COLUMNS, "username", username).await
}

pub async fn find_user_by_email(
    pool: &MySqlPool,
    email: &str,
) -> Result<Option<UserRow>, sqlx::Error> {
    find_by(pool, "users", USER_COLUMNS, "email", email).await
}

/// Single lookup used by login: matches either username or email.
pub async fn find_user_by_login(
    pool: &MySqlPool,
    login: &str,
) -> Result<Option<UserRow>, sqlx::Error> {
    let select = Select::new("users", USER_COLUMNS)
        .where_eq("username", login)
        .or_where_eq("email", login)
        .limit(1);
    find_one(pool, select).await
}

/// Insert a new user and return its generated id.
pub async fn insert_user(
    pool: &MySqlPool,
    guid: &str,
    username: &str,
    email: &str,
    password_hash: &str,
) -> Result<u64, sqlx::Error> {
    let result =
        sqlx::query("INSERT INTO users (guid, username, email, password_hash) VALUES (?, ?, ?, ?)")
            .bind(guid)
            .bind(username)
            .bind(email)
            .bind(password_hash)
            .execute(pool)
            .await?;
    Ok(result.last_insert_id())
}

/// Persist a profile update. Returns true when a row was affected.
pub async fn update_profile(
    pool: &MySqlPool,
    user_id: u64,
    username: &str,
    email: &str,
    avatar: Option<&str>,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("UPDATE users SET username = ?, email = ?, avatar = ? WHERE id = ?")
        .bind(username)
        .bind(email)
        .bind(avatar)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn update_password(
    pool: &MySqlPool,
    user_id: u64,
    password_hash: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE users SET password_hash = ? WHERE id = ?")
        .bind(password_hash)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

// --- sessions --------------------------------------------------------------
// MySQL-specific SQL (DATE_ADD, expiry comparisons) stays hand-written.

/// Session upsert + `last_logged_in` + `user.login` outbox row, in one transaction.
pub async fn record_login(
    pool: &MySqlPool,
    user_id: u64,
    session_key: &str,
    ttl_minutes: i64,
    outbox: NewOutbox<'_>,
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query(
        "INSERT INTO sessions (session_key, user_id, expires_at) \
         VALUES (?, ?, DATE_ADD(CURRENT_TIMESTAMP, INTERVAL ? MINUTE)) \
         ON DUPLICATE KEY UPDATE session_key = VALUES(session_key), \
           created_at = CURRENT_TIMESTAMP, expires_at = VALUES(expires_at)",
    )
    .bind(session_key)
    .bind(user_id)
    .bind(ttl_minutes)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE users SET last_logged_in = CURRENT_TIMESTAMP WHERE id = ?")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    insert_outbox_tx(&mut tx, &outbox).await?;
    tx.commit().await?;
    Ok(())
}

/// Find the user's session only if it has not expired.
pub async fn find_valid_session(
    pool: &MySqlPool,
    user_id: u64,
    now: NaiveDateTime,
) -> Result<Option<SessionRow>, sqlx::Error> {
    sqlx::query_as::<_, SessionRow>(
        "SELECT session_key, user_id FROM sessions WHERE user_id = ? AND ? < expires_at LIMIT 1",
    )
    .bind(user_id)
    .bind(now)
    .fetch_optional(pool)
    .await
}

/// Delete all expired sessions. Returns the number removed.
pub async fn delete_expired_sessions(
    pool: &MySqlPool,
    now: NaiveDateTime,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query("DELETE FROM sessions WHERE expires_at <= ?")
        .bind(now)
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}

// --- password reset OTPs ---------------------------------------------------

/// Upsert the OTP and enqueue the reset event, in one transaction.
pub async fn create_password_reset(
    pool: &MySqlPool,
    user_id: u64,
    code: &str,
    expires_at: NaiveDateTime,
    outbox: NewOutbox<'_>,
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query(
        "INSERT INTO password_reset_otps (user_id, code, expires_at) VALUES (?, ?, ?) \
         ON DUPLICATE KEY UPDATE code = VALUES(code), expires_at = VALUES(expires_at), created_at = CURRENT_TIMESTAMP",
    )
    .bind(user_id)
    .bind(code)
    .bind(expires_at)
    .execute(&mut *tx)
    .await?;
    insert_outbox_tx(&mut tx, &outbox).await?;
    tx.commit().await?;
    Ok(())
}

/// Return the OTP row only when the code matches and it has not expired.
pub async fn find_otp_if_valid(
    pool: &MySqlPool,
    user_id: u64,
    code: &str,
    now: NaiveDateTime,
) -> Result<Option<OtpRow>, sqlx::Error> {
    sqlx::query_as::<_, OtpRow>(
        "SELECT user_id, code, expires_at FROM password_reset_otps \
         WHERE user_id = ? AND code = ? AND ? < expires_at LIMIT 1",
    )
    .bind(user_id)
    .bind(code)
    .bind(now)
    .fetch_optional(pool)
    .await
}

/// Delete a user's OTP (called after a successful reset → single-use).
pub async fn delete_otp(pool: &MySqlPool, user_id: u64) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM password_reset_otps WHERE user_id = ?")
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

// --- outbox ----------------------------------------------------------------

/// Fetch unpublished outbox events, oldest first.
pub async fn fetch_unpublished_outbox(
    pool: &MySqlPool,
    limit: u32,
) -> Result<Vec<OutboxRow>, sqlx::Error> {
    let select = Select::new("outbox", &["id", "event_id", "topic", "payload"])
        .where_is_null("published_at")
        .order_by("id", db_core::Direction::Asc)
        .limit(limit);
    find_all(pool, select).await
}

pub async fn mark_outbox_published(pool: &MySqlPool, id: u64) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE outbox SET published_at = CURRENT_TIMESTAMP WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

async fn insert_outbox_tx(
    tx: &mut sqlx::MySqlConnection,
    outbox: &NewOutbox<'_>,
) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO outbox (event_id, topic, payload) VALUES (?, ?, ?)")
        .bind(outbox.event_id)
        .bind(outbox.topic)
        .bind(outbox.payload)
        .execute(&mut *tx)
        .await?;
    Ok(())
}
