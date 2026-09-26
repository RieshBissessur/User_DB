-- 0001_init — messenger_service: providers, formats, routing, messages.

CREATE TABLE IF NOT EXISTS providers (
  id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY,
  name VARCHAR(32) NOT NULL UNIQUE,          -- 'sendgrid' | 'smtp'
  enabled BOOLEAN NOT NULL DEFAULT TRUE,
  config JSON NOT NULL                       -- non-secret config; secrets referenced by env-var NAME
);

CREATE TABLE IF NOT EXISTS format_sendgrid (
  provider_id BIGINT UNSIGNED PRIMARY KEY,
  template_id VARCHAR(64) NOT NULL,
  dynamic_fields JSON NOT NULL,              -- placeholder -> variable mapping
  CONSTRAINT fk_fg_provider FOREIGN KEY (provider_id) REFERENCES providers(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS format_smtp (
  provider_id BIGINT UNSIGNED PRIMARY KEY,
  subject_template VARCHAR(255) NOT NULL,    -- {{username}}, {{otp}}
  body_template TEXT NOT NULL,
  CONSTRAINT fk_fs_provider FOREIGN KEY (provider_id) REFERENCES providers(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS event_routing (
  event_type VARCHAR(64) PRIMARY KEY,        -- 'user.login', 'user.password_reset_requested'
  provider_id BIGINT UNSIGNED NOT NULL,
  CONSTRAINT fk_routing_provider FOREIGN KEY (provider_id) REFERENCES providers(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS messages (
  id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY,
  event_id CHAR(36) NOT NULL UNIQUE,         -- idempotency key
  event_type VARCHAR(64) NOT NULL,
  provider_id BIGINT UNSIGNED NOT NULL,
  status ENUM('sent','failed') NOT NULL,
  detail TEXT NULL,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);