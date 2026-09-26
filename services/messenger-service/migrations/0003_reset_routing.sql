-- 0003_reset_routing — password-reset events go to SMTP (mailpit).
-- The smtp format row from 0002 already renders {{username}}/{{otp}}.

INSERT INTO event_routing (event_type, provider_id)
SELECT 'user.password_reset_requested', id FROM providers WHERE name = 'smtp'
ON DUPLICATE KEY UPDATE provider_id = VALUES(provider_id);