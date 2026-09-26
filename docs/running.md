# Running & Testing

Everything a developer needs: build, run, debug, test. Each section is
copy-pasteable.

One-time setup (after cloning):

```sh
make env        # create .env with random MySQL credentials (git-ignored)
make hooks      # optional: pre-commit fmt/clippy/test hook
```

If `docker` is not on your PATH (Rancher Desktop), either start Rancher Desktop
first, or export:

```sh
export PATH="/Applications/Rancher Desktop.app/Contents/Resources/resources/darwin/bin:$PATH"
# or use: make infra-up DOCKER="$HOME/.rd/bin/docker"
```

---

## 1. Infra in Docker, services with cargo (recommended for debugging)

```sh
make infra-up               # mysql + kafka + mailpit (waits until healthy)
make dev                    # runs BOTH services; Ctrl-C stops both
```

Then check:

```sh
curl -s localhost:3030/health       # 200
curl -s localhost:3031/health       # 200
```

Single service (two terminals):

```sh
make dev-user           # cargo run -p user-service       -> :3030
make dev-messenger      # cargo run -p messenger-service  -> :3031
```

Config comes from `.env` (created above). Logs show `[user]` / `[messenger]`
prefixes when using `make dev`.

Stop the infra when done (keeps the DB volume):

```sh
make infra-down
```

---

## 2. Full stack in Docker (no cargo needed)

```sh
make up                     # docker compose up --build -d
docker compose ps           # all containers healthy
```

Same URLs: `:3030`, `:3031`, mailpit UI `:8025`, MySQL `:3306`, Kafka `:29092`.

Tear down and **delete the databases**:

```sh
make down                   # docker compose down -v
```

---

## 3. End-to-end tests (requires the full stack)

```sh
make up                     # or: make infra-up + make dev  (infra + cargo also works)
make test-e2e
```

**Expected:** all 7 tests `ok`:

```
test duplicate_event_id_sends_exactly_one_email ... ok
test health_checks_both_services ... ok
...
test result: ok. 7 passed; 0 failed
```

You can also run them against infra + local services (Option 1):

```sh
export E2E_DATABASE_URL="mysql://root:$(grep '^MYSQL_ROOT_PASSWORD=' .env | cut -d= -f2)@127.0.0.1:3306/messenger_service"
cargo test -p e2e -- --ignored --test-threads=1
```

---

## 4. Unit & integration tests

```sh
make test-unit              # no services needed
make test-integration       # starts MySQL, runs integration tests (needs .env)
```

---

## 5. Smoke test the API by hand

```sh
# register
curl -s -X POST localhost:3030/register -H 'content-type: application/json' \
  -d '{"username":"alice","email":"alice@example.com","password":"secret123"}'
# -> {"session_key":"","username":"alice"}

# login
curl -s -X POST localhost:3030/login -H 'content-type: application/json' \
  -d '{"username":"alice","password":"secret123","version":0.1}'
# -> {"session_key":"<32 chars>","username":"alice"}

# password reset -> pick up the OTP email in mailpit, then:
curl -s -X POST localhost:3030/reset_request -H 'content-type: application/json' \
  -d '{"email":"alice@example.com"}'
# open http://localhost:8025 , read the code, then:
curl -s -X POST localhost:3030/check_otp -H 'content-type: application/json' \
  -d '{"otp":"1234","email":"alice@example.com","password":"newpass"}'
```

---

## 6. Debugging locally

Build the debug binaries, start the infra, then attach a debugger **from the
repo root** so `.env` is loaded:

```sh
make infra-up
cargo build --workspace

lldb -- target/debug/user-service
(lldb) breakpoint set --name handle_login
(lldb) run
```

Good breakpoints:

| Where | Function |
|---|---|
| user-service | `handle_register` / `handle_login` / `request_password_reset` (`routes.rs`) |
| messenger-service | `process_message` (`service.rs`), `handle_event` (`consumer.rs`) |
| shared | `Select::build` (`crates/db-core/src/query.rs`) |

**VS Code / RustRover** (CodeLLDB) — `.vscode/launch.json`:

```json
{
  "version": "0.2.0",
  "configurations": [
    {
      "type": "lldb",
      "request": "launch",
      "name": "user-service",
      "cargo": { "args": ["build", "-p", "user-service"] },
      "cwd": "${workspaceFolder}"
    },
    {
      "type": "lldb",
      "request": "launch",
      "name": "messenger-service",
      "cargo": { "args": ["build", "-p", "messenger-service"] },
      "cwd": "${workspaceFolder}"
    }
  ]
}
```

`cwd` must be the repo root so the binary finds `.env`. Prefer explicit env
values? Set them under `"env"` in the launch config instead.

Verbose logs:

```sh
RUST_LOG=debug make dev
```

Useful logs to watch:

```sh
docker compose logs -f mysql        # SQL-level issues
docker compose logs -f kafka        # broker issues
```

---

## 7. Quality gates (run before committing)

```sh
make check          # fmt --check + clippy -D warnings + no-unwrap gate
make test           # full test suite
```

The pre-commit hook (installed with `make hooks`) does the same on every
commit: fmt + `clippy --fix`, then clippy/unit-test/no-unwrap gates.
Bypass a single commit with `git commit --no-verify`.

---

## 8. Troubleshooting

| Symptom | Cause / fix |
|---|---|
| `required variable ... is missing a value` | `.env` missing → `make env` |
| `VersionMismatch` on startup | an applied migration file changed → `docker compose down -v && make infra-up` |
| MySQL exits during init | check `docker logs login_user_db-mysql-1`; init runs socket-only — see `docker/mysql/init.sh` |
| Port 3030/3031 in use | stop the local service (`make dev` Ctrl-C) or `lsof -nP -iTCP:3030 -sTCP:LISTEN` |
| `docker: command not found` | `export PATH="/Applications/Rancher Desktop.app/Contents/Resources/resources/darwin/bin:$PATH"` |
| No OTP email | messenger not running, or wrong `SMTP_HOST` in `.env`; check mailpit at <http://localhost:8025> |
| Tests fail to connect | run `make test-integration` (it reads passwords from `.env` for you) |

