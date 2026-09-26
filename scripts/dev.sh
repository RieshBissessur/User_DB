#!/usr/bin/env bash
#
# Run both services with cargo against the Docker infra (mysql/kafka/mailpit).
# Ctrl-C stops both. Start the infra first with `make infra-up`.
#
# Config (including generated DB credentials) comes from .env, which the
# services load themselves via dotenvy — create it with `make env`.

set -euo pipefail
cd "$(dirname "$0")/.."

if [ ! -f .env ]; then
    echo "no .env found — run 'make env' first" >&2
    exit 1
fi

# Give each background job its own process group so we can stop the whole tree.
set -m

cargo run -p user-service 2>&1 | awk '{printf "[user]      %s\n", $0; fflush()}' &
USER_JOB=$!
cargo run -p messenger-service 2>&1 | awk '{printf "[messenger] %s\n", $0; fflush()}' &
MESSENGER_JOB=$!

stop() {
    trap - INT TERM EXIT
    kill -- "-$USER_JOB" "-$MESSENGER_JOB" 2>/dev/null || true
    wait 2>/dev/null || true
}
trap stop EXIT
trap 'stop; exit 0' INT TERM

echo "[dev] user-service :3030   messenger-service :3031   (Ctrl-C to stop)"
wait
