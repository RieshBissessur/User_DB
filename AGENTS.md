# AGENTS.md

Guidance for agents working in this repository.

## What this is

A Cargo workspace with two `warp` + MySQL services that talk over Kafka.

| Crate | Path | Port | DB | Role |
|---|---|---|---|---|
| `user-service` | `services/user-service` | 3030 | `user_service` | accounts, sessions, password reset |
| `messenger-service` | `services/messenger-service` | 3031 | `messenger_service` | email delivery |
| `events` | `crates/events` | — | — | Kafka event contract |
| `db-core` | `crates/db-core` | — | — | schema-agnostic DB plumbing |
| `e2e` | `e2e` | — | — | cross-service tests (compose stack) |

Both services are lib + bin: `lib.rs` has `build_router(state)` and `run()`;
`main.rs` calls `run()`.

**Each service owns its schema.** SQL and row types live in
`services/<svc>/src/db/{models,repo}.rs`; migrations in `services/<svc>/migrations/`
(`pool.rs` only holds the `MIGRATOR`). `db-core` is schema-free — keep MySQL-specific
SQL (upserts, joins, `DATE_ADD`) as plain SQL in `repo.rs`.

Docs: [`docs/architecture.md`](docs/architecture.md), [`docs/setup.md`](docs/setup.md),
[`docs/running.md`](docs/running.md),
[`docs/user-service.md`](docs/user-service.md),
[`docs/messenger-service.md`](docs/messenger-service.md).

## Commands

```sh
cargo build --workspace
cargo test --workspace --lib          # unit only
make test-unit
make test-integration                 # starts MySQL, runs --tests
make test-e2e                         # full stack + e2e
make test                             # all three

make infra-up / make infra-down       # infra only (mysql, kafka, mailpit)
make dev / make dev-user / make dev-messenger   # run services with cargo
```

Migrations are embedded in each service and run on startup. Add new numbered files;
never edit an applied one.

`make hooks` activates a pre-commit hook: `cargo fmt` + `cargo clippy --fix` then
gates (clippy `-D warnings`, unit tests, `scripts/check-unwrap.sh` for
no-`unwrap`/`expect`/`panic` in production code). `make check` runs the gates
directly.

## Environment

Run `make env` to generate a git-ignored `.env` with **random MySQL credentials**
(nothing secret is committed). The services read from it:

- user-service: `DATABASE_URL`, `SERVICE_PORT`, `OTP_TTL_MINUTES`, `SESSION_TTL_MINUTES`,
  `SESSION_CLEANUP_INTERVAL_SECONDS`, `OUTBOX_POLL_MS`, `KAFKA_BROKERS`
- messenger-service: `MESSENGER_DATABASE_URL`, `MESSENGER_PORT`, `KAFKA_BROKERS`,
  `SENDGRID_API_KEY`/`SENDGRID_BASE_URL`, `SMTP_HOST`/`SMTP_PORT`/`SMTP_FROM`

## Integration testing

`#[sqlx::test]` creates and drops throwaway databases, so `DATABASE_URL` for tests must
use a privileged user:

```sh
make test-integration          # Makefile sets the root URL from .env
# or directly:
DATABASE_URL="mysql://root:$(grep '^MYSQL_ROOT_PASSWORD=' .env | cut -d= -f2)@127.0.0.1:3306/user_service" \
  cargo test --workspace --tests
```

The services themselves use least-privilege users (`user_svc` / `messenger_svc`).

## Gotchas

- **cmake is required** to build rdkafka (`brew install cmake`).
- **Docker CLI** may be off PATH on Rancher Desktop (`~/.rd/bin/docker`); the Makefile
  accepts `DOCKER=/path/to/docker`.
- **`e2e` tests are `#[ignore]`** so `cargo test --workspace` works without the stack; run
  them with `make test-e2e` (single-threaded — they share mailpit and routing).
- **SQL**: schema-specific SQL in `repo.rs`; `db-core`'s `Select` is for simple selects
  (values are binds, identifiers validated). Never interpolate a value.
- **Outbox**: handlers write the event in the same transaction as the state change;
  `workers::start_outbox_publisher` drains it. Don't publish inline.
- **Sessions** expire (`SESSION_TTL_MINUTES`) and are swept; use `find_valid_session`.
- **Passwords** are Argon2id (`password.rs`, `VARCHAR(255)`); use `verify_password`.
- **OTPs** are single-use. **One session per user** (upsert on `user_id`).
- **Events** are schema-versioned (`events::SCHEMA_VERSION`); the consumer skips newer ones.
- **Provider secrets**: `providers.config` holds `api_key_env` (production) or `api_key`
  (tests); never put real keys in the DB.
- **SMTP host**: env `SMTP_HOST` → provider `config` → default. Containers use the DB
  config (`mailpit`); host runs set `SMTP_HOST=localhost`.
- **SendGrid in compose** has no key, so `user.login` records `failed`; password-reset
  (routed to SMTP/mailpit) is the working email path.
- **Migration checksums**: editing an applied migration (even a comment) breaks startup with
  `VersionMismatch`; recreate with `docker compose down -v`.
