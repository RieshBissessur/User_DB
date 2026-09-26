# Setup

Two ways to run locally: **infra in Docker + services with `cargo`** (best for
debugging) or **everything in Docker**. The detailed run/test/debug walkthrough
lives in [`running.md`](running.md); this page is just setup.

## Prerequisites

- **Rust** (stable)
- **cmake** — builds the Kafka clients: `brew install cmake`
- **Docker** — [Rancher Desktop](https://rancherdesktop.io) on macOS (or any Docker)
- **make** (optional, for the shortcuts)

---

## Option A — infra in Docker, services with cargo (recommended)

### 1. Environment

```sh
make env                  # writes git-ignored .env with RANDOM MySQL credentials
```

Every clone gets unique credentials — nothing secret is committed. The services
load `.env` from the working directory, so run `cargo` commands from the repo root.

### 2. Install the pre-commit hook (recommended)

```sh
make hooks
```

On each commit it runs `cargo fmt` (auto-fixes and re-stages your staged `.rs`
files), `cargo clippy --fix`, then gates on: clippy `-D warnings`, unit tests,
and a no-`unwrap` check on production code. Bypass one commit with
`git commit --no-verify`; run the same checks any time with `make check`.

### 3. Start the infra

```sh
make infra-up             # mysql + kafka + mailpit, waits until healthy
# or: docker compose up -d mysql kafka mailpit
```

### 4. Run the services

```sh
make dev                  # both services, prefixed logs, Ctrl-C stops both
```

or one at a time (two terminals):

```sh
make dev-user             # cargo run -p user-service      -> :3030
make dev-messenger        # cargo run -p messenger-service -> :3031
```

Config is read from `.env`; no exports needed. `make dev` only *defaults* the
variables if `.env` is missing.

### 5. Smoke test

```sh
curl -s localhost:3030/health
curl -s localhost:3031/health

curl -s -X POST localhost:3030/register -H 'content-type: application/json' \
  -d '{"username":"alice","email":"alice@example.com","password":"secret123"}'
```

Password-reset emails land in mailpit: <http://localhost:8025>.

### Debugging

Full walkthrough (lldb, VS Code launch config, breakpoints, verbose logs):
[`running.md`](running.md#6-debugging-locally). Short version:

```sh
make infra-up
cargo build --workspace
lldb -- target/debug/user-service
(lldb) breakpoint set --name handle_login
(lldb) run
```

---

## Option B — everything in Docker

```sh
docker compose up --build        # add -d to detach
```

| Service / tool | URL / port |
|---|---|
| user-service | http://localhost:3030 |
| messenger-service | http://localhost:3031 |
| MySQL | localhost:3306 |
| Kafka (host) | localhost:29092 (in-network `kafka:9092`) |
| mailpit UI | http://localhost:8025 |
| kafka-ui (optional) | http://localhost:8080 — `docker compose --profile debug up` |

Tear down and delete the databases:

```sh
docker compose down -v
```

---

## Tests

Quick reference:

```sh
make test              # unit + integration + e2e
make test-unit         # pure logic, no services
make test-integration  # starts MySQL, runs service integration tests
make test-e2e          # builds + starts the full stack, runs the e2e crate
```

Verifying the e2e run, running the API by hand, and the troubleshooting table
live in [`running.md`](running.md).

---

## Scripts

| Script | What it does |
|---|---|
| `scripts/init-env.sh` | generate `.env` with random DB credentials (`make env`) |
| `scripts/dev.sh` | run both services from cargo with prefixed logs |
| `scripts/check-unwrap.sh` | gate: no `unwrap`/`expect`/`panic!` in non-test code |
| `.githooks/pre-commit` | fmt + `clippy --fix` + clippy/unit/unwrap gates (via `make hooks`) |

## Troubleshooting

- **`VersionMismatch` on startup** — an applied migration's bytes changed. Recreate the DB:
  `docker compose down -v && docker compose up -d mysql`.
- **`DATABASE_URL is not set`** — run from the repo root so `.env` is found; create
  it with `make env`.
- **`docker: command not found`** — Rancher Desktop's CLI may be off PATH; use
  `~/.rd/bin/docker`, e.g. `make infra-up DOCKER="$HOME/.rd/bin/docker"`.
- **Port already in use** — stop the local service on `3030`/`3031` or the container.
- **No reset email** — check mailpit at <http://localhost:8025> and that the messenger
  service is running.
- More: [`running.md` § Troubleshooting](running.md#8-troubleshooting).
