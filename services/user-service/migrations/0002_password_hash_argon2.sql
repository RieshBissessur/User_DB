-- 0002_password_hash_argon2 — widen users.password_hash for Argon2 PHC strings.
-- Argon2 PHC hashes are ~97 chars, so SHA-256's CHAR(64) is too small.
-- Fresh start: no legacy hashes need re-verification.
ALTER TABLE users MODIFY COLUMN password_hash VARCHAR(255) NOT NULL;