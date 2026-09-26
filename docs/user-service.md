# user-service

Accounts, sessions and password reset. `warp` on **:3030**, database **user_service**.
Code: `services/user-service` (SQL in `src/db/repo.rs`, migrations in `migrations/`).

## API

| Method | Path | Body | Notes |
|---|---|---|---|
| GET | `/health` | — | 200 when healthy |
| POST | `/register` | `{username, email, password}` | usernames/emails stored lowercase; returns empty `session_key` |
| POST | `/login` | `{username, password, version}` | `version >= 0.1`; returns `{session_key, username}`; replaces the user's session |
| POST | `/user_data` | `{username, session_key}` | returns `{username, email, avatar}` |
| POST | `/update_user_data` | `{username, new_username?, email?, avatar?, session_key}` | persists the change |
| POST | `/reset_request` | `{email}` | creates a 4-digit OTP and publishes a reset event |
| POST | `/check_otp` | `{otp, email, password}` | sets the new password and deletes the OTP |

Errors return `400` with a plain-text reason (the legacy `CustomRejection` style).

## Login flow

```mermaid
sequenceDiagram
    participant C as Client
    participant U as user-service
    participant DB as user_service
    participant P as Outbox poller
    participant K as Kafka

    C->>U: POST /login
    U->>U: verify Argon2id hash + version
    U->>DB: session + last_logged_in + outbox(user.login) in one tx
    U-->>C: 200 {session_key}
    P->>DB: read unpublished outbox
    P->>K: publish user-events
```

## Password reset flow

```mermaid
sequenceDiagram
    participant C as Client
    participant U as user-service
    participant DB as user_service
    participant M as messenger-service

    C->>U: POST /reset_request {email}
    U->>DB: OTP + outbox(reset event w/ otp) in one tx
    U-->>C: 200 "OTP Sent to email address"
    M->>C: (delivers OTP email via messenger)
    C->>U: POST /check_otp {otp, email, new_password}
    U->>DB: verify OTP, set password, delete OTP
    U-->>C: 200 "OTP match and valid"
```

## Data model

```mermaid
erDiagram
    users {
        BIGINT id PK
        CHAR guid UK
        VARCHAR username UK
        VARCHAR email UK
        VARCHAR password_hash
        VARCHAR avatar
        DATETIME last_logged_in
        DATETIME created_at
        DATETIME updated_at
    }
    sessions {
        CHAR session_key PK
        BIGINT user_id FK
        DATETIME created_at
        DATETIME expires_at
    }
    password_reset_otps {
        BIGINT user_id PK
        CHAR code
        DATETIME expires_at
    }
    outbox {
        BIGINT id PK
        CHAR event_id UK
        VARCHAR topic
        JSON payload
        DATETIME published_at
    }
    users ||--o| sessions : "one"
    users ||--o| password_reset_otps : "one"
```

## Config

| Variable | Default | Purpose |
|---|---|---|
| `DATABASE_URL` | — | MySQL connection (required) |
| `SERVICE_PORT` | `3030` | HTTP port |
| `OTP_TTL_MINUTES` | `120` | OTP validity |
| `SESSION_TTL_MINUTES` | `10080` | session lifetime |
| `SESSION_CLEANUP_INTERVAL_SECONDS` | `3600` | expired-session sweep |
| `OUTBOX_POLL_MS` | `1000` | outbox publish interval |
| `KAFKA_BROKERS` | `kafka:9092` | Kafka bootstrap |
| `RUST_LOG` | `info` | log filter |

## Behaviour notes

- **Argon2id** password hashing (`password.rs`).
- **One session per user** (upsert); expired sessions rejected and swept by a worker.
- OTPs are **single-use**.
- Login/reset never fail because Kafka is down — the event waits in the outbox.

## Run & debug this service

```sh
make infra-up      # once
make dev-user      # cargo run -p user-service -> :3030
```

Breakpoints on `handle_register` / `handle_login` / `request_password_reset`
(`routes.rs`); full walkthrough in [`running.md`](running.md).
