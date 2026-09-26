//! HTTP integration tests: `warp::test` + real MySQL (`#[sqlx::test]`).
//!
//!   DATABASE_URL="mysql://root:<MYSQL_ROOT_PASSWORD>@127.0.0.1:3306/user_service" \
//!     cargo test -p user-service --tests
//! (or `make test-integration` — it reads the password from .env)

use chrono::NaiveDateTime;
use serde_json::{json, Value};
use sqlx::MySqlPool;
use user_service::routes::{build_router, AppState};

const APP_VERSION: f32 = 0.1;

/// A test state with no Kafka producer (publishing is skipped) so no external
/// services are needed. The OTP is read straight from the DB in the reset test.
fn test_state(pool: MySqlPool) -> AppState {
    AppState {
        pool,
        otp_ttl_minutes: 120,
        session_ttl_minutes: 10080,
        app_version: APP_VERSION,
        producer: None,
    }
}

fn as_json(raw: &str) -> Value {
    serde_json::from_str(raw).unwrap_or(Value::Null)
}

/// POST `body` to `path` and return `(status, raw_body)`.
/// Error responses are HTML, success responses are JSON, so callers parse when
/// they expect JSON.
macro_rules! post {
    ($api:expr, $path:expr, $body:expr) => {{
        let res = warp::test::request()
            .method("POST")
            .path($path)
            .json(&$body)
            .reply($api)
            .await;
        (
            res.status().as_u16(),
            String::from_utf8_lossy(res.body()).to_string(),
        )
    }};
}

macro_rules! register {
    ($api:expr, $username:expr, $email:expr, $password:expr) => {
        post!(
            $api,
            "/register",
            json!({ "username": $username, "email": $email, "password": $password })
        )
        .0
    };
}

macro_rules! register_and_login {
    ($pool:expr, $username:expr, $email:expr, $password:expr) => {{
        let api = build_router(test_state($pool.clone()));
        assert_eq!(register!(&api, $username, $email, $password), 200);
        let (status, raw) = post!(
            &api,
            "/login",
            json!({ "username": $username, "password": $password, "version": APP_VERSION })
        );
        assert_eq!(status, 200, "login should succeed");
        (api, as_json(&raw)["session_key"].as_str().unwrap().to_string())
    }};
}

#[sqlx::test(migrator = "user_service::db::MIGRATOR")]
async fn register_login_user_data_happy_path(pool: MySqlPool) {
    let api = build_router(test_state(pool.clone()));

    let (status, raw) = post!(
        &api,
        "/register",
        json!({ "username": "Alice", "email": "Alice@Example.com", "password": "secret123" })
    );
    assert_eq!(status, 200);
    let body = as_json(&raw);
    assert_eq!(body["session_key"], "");
    assert_eq!(body["username"], "alice"); // normalized on write

    let (status, raw) = post!(
        &api,
        "/login",
        json!({ "username": "alice", "password": "secret123", "version": APP_VERSION })
    );
    assert_eq!(status, 200);
    let session_key = as_json(&raw)["session_key"].as_str().unwrap().to_string();
    assert_eq!(session_key.len(), 32);

    let (status, raw) = post!(
        &api,
        "/user_data",
        json!({ "username": "alice", "session_key": session_key })
    );
    assert_eq!(status, 200);
    let body = as_json(&raw);
    assert_eq!(body["username"], "alice");
    assert_eq!(body["email"], "alice@example.com");
}

#[sqlx::test(migrator = "user_service::db::MIGRATOR")]
async fn login_with_email_works(pool: MySqlPool) {
    let api = build_router(test_state(pool.clone()));
    register!(&api, "bob", "bob@example.com", "pw");

    let (status, _) = post!(
        &api,
        "/login",
        json!({ "username": "bob@example.com", "password": "pw", "version": APP_VERSION })
    );
    assert_eq!(status, 200);
}

#[sqlx::test(migrator = "user_service::db::MIGRATOR")]
async fn duplicate_username_rejected(pool: MySqlPool) {
    let api = build_router(test_state(pool.clone()));
    assert_eq!(register!(&api, "carol", "carol@example.com", "pw"), 200);
    assert_eq!(register!(&api, "Carol", "carol2@example.com", "pw"), 400);
}

#[sqlx::test(migrator = "user_service::db::MIGRATOR")]
async fn duplicate_email_rejected(pool: MySqlPool) {
    let api = build_router(test_state(pool.clone()));
    assert_eq!(register!(&api, "dave", "dave@example.com", "pw"), 200);
    assert_eq!(register!(&api, "dave2", "DAVE@example.com", "pw"), 400);
}

#[sqlx::test(migrator = "user_service::db::MIGRATOR")]
async fn wrong_password_rejected(pool: MySqlPool) {
    let api = build_router(test_state(pool.clone()));
    register!(&api, "erin", "erin@example.com", "correct");

    let (status, _) = post!(
        &api,
        "/login",
        json!({ "username": "erin", "password": "wrong", "version": APP_VERSION })
    );
    assert_eq!(status, 400);
}

#[sqlx::test(migrator = "user_service::db::MIGRATOR")]
async fn bad_session_key_rejected(pool: MySqlPool) {
    let api = build_router(test_state(pool.clone()));
    register!(&api, "frank", "frank@example.com", "pw");

    let (status, _) = post!(
        &api,
        "/user_data",
        json!({ "username": "frank", "session_key": "not-the-key" })
    );
    assert_eq!(status, 400);
}

#[sqlx::test(migrator = "user_service::db::MIGRATOR")]
async fn missing_session_key_rejected(pool: MySqlPool) {
    let api = build_router(test_state(pool.clone()));
    register!(&api, "gina", "gina@example.com", "pw");

    // No login → no session row at all.
    let (status, _) = post!(
        &api,
        "/user_data",
        json!({ "username": "gina", "session_key": "anything" })
    );
    assert_eq!(status, 400);
}

#[sqlx::test(migrator = "user_service::db::MIGRATOR")]
async fn version_gate_rejects_old_client(pool: MySqlPool) {
    let api = build_router(test_state(pool.clone()));
    register!(&api, "henry", "henry@example.com", "pw");

    let (status, raw) = post!(
        &api,
        "/login",
        json!({ "username": "henry", "password": "pw", "version": 0.0 })
    );
    assert_eq!(status, 400);
    assert!(
        raw.contains("Please update application version"),
        "raw={raw}"
    );
}

#[sqlx::test(migrator = "user_service::db::MIGRATOR")]
async fn update_user_data_persists(pool: MySqlPool) {
    let (api, session) = register_and_login!(&pool, "ivy", "ivy@example.com", "pw");

    let (status, raw) = post!(
        &api,
        "/update_user_data",
        json!({
            "username": "ivy",
            "new_username": "ivy_new",
            "email": "ivy_new@example.com",
            "avatar": "https://example.com/a.png",
            "session_key": session
        })
    );
    assert_eq!(status, 200);
    let body = as_json(&raw);
    assert_eq!(body["username"], "ivy_new");
    assert_eq!(body["email"], "ivy_new@example.com");

    // Re-read through the API using the renamed login and the same session.
    let (status, raw) = post!(
        &api,
        "/user_data",
        json!({ "username": "ivy_new", "session_key": session })
    );
    assert_eq!(status, 200);
    let body = as_json(&raw);
    assert_eq!(body["username"], "ivy_new");
    assert_eq!(body["email"], "ivy_new@example.com");
    assert_eq!(body["avatar"], "https://example.com/a.png");
}

#[sqlx::test(migrator = "user_service::db::MIGRATOR")]
async fn reset_flow_is_single_use(pool: MySqlPool) {
    let api = build_router(test_state(pool.clone()));
    register!(&api, "jack", "jack@example.com", "oldpass");

    let (status, _) = post!(
        &api,
        "/reset_request",
        json!({ "email": "jack@example.com" })
    );
    assert_eq!(status, 200);

    let code: String = sqlx::query_scalar(
        "SELECT code FROM password_reset_otps WHERE user_id = (SELECT id FROM users WHERE email = ?)",
    )
    .bind("jack@example.com")
    .fetch_one(&pool)
    .await
    .expect("OTP row should exist after reset_request");
    assert_eq!(code.len(), 4);

    let (status, raw) = post!(
        &api,
        "/check_otp",
        json!({ "otp": code, "email": "jack@example.com", "password": "newpass" })
    );
    assert_eq!(status, 200);
    assert!(raw.contains("OTP match and valid"), "raw={raw}");

    // Row deleted → single-use.
    let remaining: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM password_reset_otps WHERE user_id = (SELECT id FROM users WHERE email = ?)",
    )
    .bind("jack@example.com")
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(remaining, 0);

    // New password works, old one does not.
    let (status, _) = post!(
        &api,
        "/login",
        json!({ "username": "jack", "password": "newpass", "version": APP_VERSION })
    );
    assert_eq!(status, 200);

    let (status, _) = post!(
        &api,
        "/login",
        json!({ "username": "jack", "password": "oldpass", "version": APP_VERSION })
    );
    assert_eq!(status, 400);

    // Reusing the consumed OTP fails.
    let (status, raw) = post!(
        &api,
        "/check_otp",
        json!({ "otp": code, "email": "jack@example.com", "password": "again" })
    );
    assert_eq!(status, 200);
    assert!(raw.contains("OTP invalid or expired"), "raw={raw}");
}

#[sqlx::test(migrator = "user_service::db::MIGRATOR")]
async fn reset_request_unknown_email_rejected(pool: MySqlPool) {
    let api = build_router(test_state(pool.clone()));
    let (status, _) = post!(
        &api,
        "/reset_request",
        json!({ "email": "nobody@example.com" })
    );
    assert_eq!(status, 400);
}

#[sqlx::test(migrator = "user_service::db::MIGRATOR")]
async fn last_logged_in_advances_on_second_login(pool: MySqlPool) {
    let api = build_router(test_state(pool.clone()));
    register!(&api, "kim", "kim@example.com", "pw");

    let login = json!({ "username": "kim", "password": "pw", "version": APP_VERSION });
    post!(&api, "/login", login.clone());
    let first: Option<NaiveDateTime> =
        sqlx::query_scalar("SELECT last_logged_in FROM users WHERE username = 'kim'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(first.is_some(), "last_logged_in should be set on login");

    // MySQL DATETIME has second precision; wait for the clock to tick.
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    post!(&api, "/login", login);
    let second: Option<NaiveDateTime> =
        sqlx::query_scalar("SELECT last_logged_in FROM users WHERE username = 'kim'")
            .fetch_one(&pool)
            .await
            .unwrap();

    assert!(second > first, "second login should advance last_logged_in");
}

#[sqlx::test(migrator = "user_service::db::MIGRATOR")]
async fn expired_session_is_rejected_and_swept(pool: MySqlPool) {
    let (api, session) = register_and_login!(&pool, "lena", "lena@example.com", "pw");

    // Force the session into the past.
    sqlx::query(
        "UPDATE sessions SET expires_at = DATE_SUB(CURRENT_TIMESTAMP, INTERVAL 1 MINUTE) WHERE session_key = ?",
    )
    .bind(&session)
    .execute(&pool)
    .await
    .unwrap();

    let (status, _) = post!(
        &api,
        "/user_data",
        json!({ "username": "lena", "session_key": session })
    );
    assert_eq!(status, 400, "expired session must be rejected");

    // Cleanup sweep removes it.
    let removed =
        user_service::db::delete_expired_sessions(&pool, chrono::Local::now().naive_local())
            .await
            .unwrap();
    assert_eq!(removed, 1);
}

#[sqlx::test(migrator = "user_service::db::MIGRATOR")]
async fn login_enqueues_outbox_event(pool: MySqlPool) {
    let _ = register_and_login!(&pool, "mara", "mara@example.com", "pw");

    let (topic, payload): (String, serde_json::Value) = sqlx::query_as(
        "SELECT topic, payload FROM outbox WHERE published_at IS NULL ORDER BY id DESC LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .expect("login should enqueue an outbox row");

    assert_eq!(topic, "user-events");
    assert_eq!(payload["event_type"], "user.login");
    assert_eq!(payload["username"], "mara");
}

#[sqlx::test(migrator = "user_service::db::MIGRATOR")]
async fn reset_request_enqueues_outbox_event(pool: MySqlPool) {
    let api = build_router(test_state(pool.clone()));
    register!(&api, "nina", "nina@example.com", "pw");

    let (status, _) = post!(
        &api,
        "/reset_request",
        json!({ "email": "nina@example.com" })
    );
    assert_eq!(status, 200);

    let (topic, payload): (String, serde_json::Value) = sqlx::query_as(
        "SELECT topic, payload FROM outbox WHERE published_at IS NULL ORDER BY id DESC LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .expect("reset_request should enqueue an outbox row");

    assert_eq!(topic, "user-events");
    assert_eq!(payload["event_type"], "user.password_reset_requested");
    assert!(payload["payload"]["otp"].as_str().unwrap().len() == 4);
}

/// The shared `db-core` helpers return *any* `FromRow` struct; exercise
/// `find_by_id` against a real MySQL row.
#[sqlx::test(migrator = "user_service::db::MIGRATOR")]
async fn db_core_find_by_id_returns_a_row(pool: MySqlPool) {
    let api = build_router(test_state(pool.clone()));
    register!(&api, "oscar", "oscar@example.com", "pw");

    let id: u64 = sqlx::query_scalar("SELECT id FROM users WHERE username = 'oscar'")
        .fetch_one(&pool)
        .await
        .expect("registered user should exist");

    let user: user_service::db::UserRow = db_core::find_by_id(
        &pool,
        "users",
        &[
            "id",
            "guid",
            "username",
            "email",
            "password_hash",
            "avatar",
            "last_logged_in",
        ],
        id,
    )
    .await
    .expect("query should succeed")
    .expect("user row should exist");

    assert_eq!(user.id, id);
    assert_eq!(user.username, "oscar");
}
