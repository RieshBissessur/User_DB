use rdkafka::config::ClientConfig;
use rdkafka::consumer::{CommitMode, Consumer, StreamConsumer};
use rdkafka::message::Message;

use events::{Event, MESSENGER_CONSUMER_GROUP, USER_EVENTS_TOPIC};

use crate::service::{process_message, AppState};

/// Spawn the Kafka consumer as a background task.
pub fn start_consumer(state: AppState, brokers: &str) {
    let brokers = brokers.to_string();
    tokio::spawn(async move {
        run_consumer(state, &brokers).await;
    });
}

async fn run_consumer(state: AppState, brokers: &str) {
    let consumer: StreamConsumer = match ClientConfig::new()
        .set("bootstrap.servers", brokers)
        .set("group.id", MESSENGER_CONSUMER_GROUP)
        .set("enable.auto.commit", "false")
        .set("auto.offset.reset", "earliest")
        .set("session.timeout.ms", "10000")
        .set("max.poll.interval.ms", "300000")
        .set("enable.partition.eof", "false")
        .set("fetch.min.bytes", "1")
        .create()
    {
        Ok(consumer) => consumer,
        Err(err) => {
            tracing::error!(error = %err, "failed to create Kafka consumer");
            return;
        }
    };

    if let Err(err) = consumer.subscribe(&[USER_EVENTS_TOPIC]) {
        tracing::error!(error = %err, "failed to subscribe to {USER_EVENTS_TOPIC}");
        return;
    }

    tracing::info!(brokers, topic = USER_EVENTS_TOPIC, "consumer started");
    loop {
        match consumer.recv().await {
            Ok(message) => {
                let payload = match message.payload_view::<str>() {
                    Some(Ok(text)) => text,
                    _ => {
                        tracing::warn!("received non-UTF8 Kafka payload; skipping");
                        let _ = consumer.commit_message(&message, CommitMode::Async);
                        continue;
                    }
                };

                match Event::from_json(payload) {
                    Ok(event) => match event.validate() {
                        Ok(()) => handle_event(&state, &event).await,
                        Err(err) => {
                            // Skip newer schemas.
                            tracing::warn!(error = %err, "ignoring event with unsupported schema");
                        }
                    },
                    Err(err) => {
                        // Log and skip malformed events.
                        tracing::warn!(error = %err, "ignoring unknown/malformed event");
                    }
                }

                // At-least-once; re-delivery is safe (idempotent on event_id).
                if let Err(err) = consumer.commit_message(&message, CommitMode::Async) {
                    tracing::warn!(error = %err, "failed to commit offset");
                }
            }
            Err(err) => {
                tracing::warn!(error = %err, "kafka receive error");
            }
        }
    }
}

/// Handle an event: route → send → record.
pub async fn handle_event(state: &AppState, event: &Event) {
    let variables = events::variables_for(event);
    match process_message(
        state,
        &event.event_id,
        event.event_type.as_str(),
        &event.email,
        &variables,
    )
    .await
    {
        Ok(response) => {
            tracing::info!(event_id = %event.event_id, status = %response.status, "handled event")
        }
        Err(err) => {
            tracing::error!(event_id = %event.event_id, error = %err, "failed to handle event")
        }
    }
}
