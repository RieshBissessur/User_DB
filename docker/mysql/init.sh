#!/usr/bin/env sh
# Runs on first MySQL container boot (empty data volume).
# Credentials come from the environment (see docker-compose.yml / .env) — no
# secrets are baked into this file.
#
# During initdb a temporary server runs socket-only, so connect without -h.
set -eu

mysql -uroot -p"$MYSQL_ROOT_PASSWORD" <<SQL
CREATE DATABASE IF NOT EXISTS user_service;
CREATE DATABASE IF NOT EXISTS messenger_service;

CREATE USER IF NOT EXISTS 'user_svc'@'%' IDENTIFIED BY '${USER_SVC_PASSWORD}';
GRANT ALL PRIVILEGES ON user_service.* TO 'user_svc'@'%';

CREATE USER IF NOT EXISTS 'messenger_svc'@'%' IDENTIFIED BY '${MESSENGER_SVC_PASSWORD}';
GRANT ALL PRIVILEGES ON messenger_service.* TO 'messenger_svc'@'%';

FLUSH PRIVILEGES;
SQL
