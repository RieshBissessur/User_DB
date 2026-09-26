-- 0003_session_expiry — sessions carry a hard expiry and are swept by a worker.
ALTER TABLE sessions
  ADD COLUMN expires_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP;

CREATE INDEX idx_sessions_expires ON sessions (expires_at);