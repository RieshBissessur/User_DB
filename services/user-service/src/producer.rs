use std::time::Duration;

use rdkafka::config::ClientConfig;
use rdkafka::producer::{FutureProducer, FutureRecord};

/// Kafka producer tuned for durability (`acks=all`, idempotent, gzip).
/// Connects lazily, so an unreachable broker never blocks startup.
#[derive(Clone)]
pub struct Producer {
    inner: FutureProducer,
}

impl Producer {
    pub fn new(brokers: &str) -> Result<Self, rdkafka::error::KafkaError> {
        let inner: FutureProducer = ClientConfig::new()
            .set("bootstrap.servers", brokers)
            .set("acks", "all")
            .set("enable.idempotence", "true")
            .set("retries", "2147483647")
            .set("linger.ms", "10")
            .set("compression.type", "gzip")
            .set("message.timeout.ms", "5000")
            .create()?;
        Ok(Self { inner })
    }

    /// Publish `payload` to `topic`. Failure is non-fatal (the outbox retries).
    pub async fn publish_payload(
        &self,
        topic: &str,
        key: &str,
        payload: &str,
    ) -> Result<(), String> {
        let record = FutureRecord::to(topic).key(key).payload(payload);
        self.inner
            .send(record, Duration::from_secs(5))
            .await
            .map(|_| ())
            .map_err(|(err, _)| err.to_string())
    }
}
