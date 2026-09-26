use sqlx::mysql::MySqlPool;

use db_core::find_by;

use super::models::{ProviderRow, SendgridFormatRow, SmtpFormatRow};

// --- routing / providers ---------------------------------------------------

/// Provider routed for an event type (even if disabled). Join stays plain SQL.
pub async fn find_provider_for_event(
    pool: &MySqlPool,
    event_type: &str,
) -> Result<Option<ProviderRow>, sqlx::Error> {
    sqlx::query_as::<_, ProviderRow>(
        "SELECT p.id, p.name, p.enabled, p.config \
         FROM event_routing r JOIN providers p ON p.id = r.provider_id \
         WHERE r.event_type = ? LIMIT 1",
    )
    .bind(event_type)
    .fetch_optional(pool)
    .await
}

// --- formats ---------------------------------------------------------------

pub async fn find_sendgrid_format(
    pool: &MySqlPool,
    provider_id: u64,
) -> Result<Option<SendgridFormatRow>, sqlx::Error> {
    find_by(
        pool,
        "format_sendgrid",
        &["provider_id", "template_id", "dynamic_fields"],
        "provider_id",
        provider_id,
    )
    .await
}

pub async fn find_smtp_format(
    pool: &MySqlPool,
    provider_id: u64,
) -> Result<Option<SmtpFormatRow>, sqlx::Error> {
    find_by(
        pool,
        "format_smtp",
        &["provider_id", "subject_template", "body_template"],
        "provider_id",
        provider_id,
    )
    .await
}

// --- messages (delivery log + idempotency) ---------------------------------

pub async fn message_exists(pool: &MySqlPool, event_id: &str) -> Result<bool, sqlx::Error> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM messages WHERE event_id = ?")
        .bind(event_id)
        .fetch_one(pool)
        .await?;
    Ok(count > 0)
}

/// Record delivery; `event_id` is unique (idempotent).
pub async fn insert_message(
    pool: &MySqlPool,
    event_id: &str,
    event_type: &str,
    provider_id: u64,
    status: &str,
    detail: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO messages (event_id, event_type, provider_id, status, detail) VALUES (?, ?, ?, ?, ?) \
         ON DUPLICATE KEY UPDATE status = VALUES(status), detail = VALUES(detail)",
    )
    .bind(event_id)
    .bind(event_type)
    .bind(provider_id)
    .bind(status)
    .bind(detail)
    .execute(pool)
    .await?;
    Ok(())
}
