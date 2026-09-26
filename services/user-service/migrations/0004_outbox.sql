-- 0004_outbox — transactional outbox for at-least-once Kafka delivery.
-- Events are written in the same transaction as the state change and published
-- to Kafka afterwards by a poller.
CREATE TABLE IF NOT EXISTS outbox (
  id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY,
  event_id CHAR(36) NOT NULL UNIQUE,
  topic VARCHAR(128) NOT NULL,
  payload JSON NOT NULL,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  published_at DATETIME NULL,
  KEY idx_outbox_unpublished (published_at)
);