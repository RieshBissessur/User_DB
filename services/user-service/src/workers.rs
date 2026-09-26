use std::time::Duration;

use chrono::Local;
use sqlx::MySqlPool;

use crate::producer::Producer;
use crate::routes::AppState;

/// Periodically delete expired sessions.
pub fn start_session_cleanup(pool: MySqlPool, interval_seconds: u64) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(interval_seconds.max(1)));
        loop {
            ticker.tick().await;
            let now = Local::now().naive_local();
            match crate::db::delete_expired_sessions(&pool, now).await {
                Ok(0) => {}
                Ok(n) => tracing::info!(removed = n, "swept expired sessions"),
                Err(err) => tracing::warn!(error = %err, "session cleanup failed"),
            }
        }
    });
}

/// Poll the outbox and publish pending events. Failures retry next tick.
pub fn start_outbox_publisher(state: AppState, poll_ms: u64) {
    let producer = match state.producer.clone() {
        Some(producer) => producer,
        None => return,
    };
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_millis(poll_ms.max(50)));
        loop {
            ticker.tick().await;
            if let Err(err) = drain_once(&state.pool, &producer).await {
                tracing::warn!(error = %err, "outbox drain failed");
            }
        }
    });
}

async fn drain_once(pool: &MySqlPool, producer: &Producer) -> Result<(), sqlx::Error> {
    let pending = crate::db::fetch_unpublished_outbox(pool, 100).await?;
    for row in pending {
        let payload = match serde_json::to_string(&row.payload) {
            Ok(payload) => payload,
            Err(err) => {
                tracing::error!(event_id = %row.event_id, error = %err, "invalid outbox payload");
                // Poison payload: mark published so it cannot stall the queue.
                crate::db::mark_outbox_published(pool, row.id).await?;
                continue;
            }
        };
        match producer
            .publish_payload(&row.topic, &row.event_id, &payload)
            .await
        {
            Ok(()) => {
                crate::db::mark_outbox_published(pool, row.id).await?;
                tracing::info!(event_id = %row.event_id, topic = %row.topic, "published outbox event");
            }
            Err(err) => {
                tracing::warn!(event_id = %row.event_id, error = %err, "publish failed; will retry");
                break; // preserve ordering; retry the batch next tick
            }
        }
    }
    Ok(())
}
