-- 0001_init — user_service: users, sessions, password-reset OTPs
-- Fresh start: no migration of legacy JSON file data.

CREATE TABLE IF NOT EXISTS users (
  id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY,
  guid CHAR(36) NOT NULL UNIQUE,
  username VARCHAR(64) NOT NULL,
  email VARCHAR(255) NOT NULL,
  password_hash CHAR(64) NOT NULL,          -- unsalted SHA-256 for now (argon2 = follow-up)
  avatar VARCHAR(512) NULL,
  last_logged_in DATETIME NULL,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  UNIQUE KEY uq_username (username),
  UNIQUE KEY uq_email (email)
);

CREATE TABLE IF NOT EXISTS sessions (
  session_key CHAR(32) PRIMARY KEY,          -- random 32-char, as today
  user_id BIGINT UNSIGNED NOT NULL,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  UNIQUE KEY uq_session_user (user_id),      -- one-session-per-user preserved (upsert)
  CONSTRAINT fk_sessions_user FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS password_reset_otps (
  user_id BIGINT UNSIGNED NOT NULL PRIMARY KEY,
  code CHAR(4) NOT NULL,
  expires_at DATETIME NOT NULL,              -- now() + OTP_TTL_MINUTES (default 120)
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  CONSTRAINT fk_otps_user FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);