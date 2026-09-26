-- 0002_seed — default providers, formats and routing.
-- Password-reset routing (→ smtp) is added in Phase 5.

INSERT INTO providers (name, enabled, config) VALUES
  ('sendgrid', TRUE, JSON_OBJECT(
      'from_email', 'no-reply@pattern-shop.local',
      'api_key_env', 'SENDGRID_API_KEY',
      'template_id', 'd-36dab063ce184e4180e716439b12ac9a'
  )),
  ('smtp', TRUE, JSON_OBJECT(
      'host', 'mailpit',
      'port', 1025,
      'from_email', 'no-reply@pattern-shop.local'
  ));

INSERT INTO format_sendgrid (provider_id, template_id, dynamic_fields)
SELECT id, 'd-36dab063ce184e4180e716439b12ac9a',
       JSON_OBJECT('username', 'username', 'otp', 'otp', 'email', 'email')
FROM providers WHERE name = 'sendgrid';

INSERT INTO format_smtp (provider_id, subject_template, body_template)
SELECT id, 'Your verification code',
       'Hi {{username}}, your verification code is {{otp}}.'
FROM providers WHERE name = 'smtp';

INSERT INTO event_routing (event_type, provider_id)
SELECT 'user.login', id FROM providers WHERE name = 'sendgrid';