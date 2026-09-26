//! Messenger integration tests: `warp::test` + real MySQL, wiremock SendGrid,
//! stub SMTP transport.
//!
//!   DATABASE_URL="mysql://root:<MYSQL_ROOT_PASSWORD>@127.0.0.1:3306/messenger_service" \
//!     cargo test -p messenger-service --tests
//! (or `make test-integration`; the password lives in .env via `make env`)

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use lettre::transport::stub::StubTransport;
use serde_json::{json, Value};
use sqlx::MySqlPool;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use messenger_service::provider::EmailTransport;
use messenger_service::routes::build_router;
use messenger_service::service::{AppState, Registry};
use messenger_service::{MessageSender, SendError};

fn state(pool: MySqlPool, smtp_transport: Option<Arc<dyn EmailTransport>>) -> AppState {
    AppState {
        pool,
        registry: Registry {
            sendgrid_base_url: "http://127.0.0.1:1".to_string(),
            smtp_from: "no-reply@test.local".to_string(),
            smtp_host_override: None,
            smtp_port_override: None,
            smtp_transport_override: smtp_transport,
            sender_override: None,
        },
    }
}

/// A mock `MessageSender` that records what the consumer/handler tried to send.
type SentLog = Arc<Mutex<Vec<(String, HashMap<String, String>)>>>;

#[derive(Default)]
struct RecordingSender {
    sent: SentLog,
}

#[async_trait]
impl MessageSender for RecordingSender {
    async fn send(&self, to: &str, variables: &HashMap<String, String>) -> Result<(), SendError> {
        self.sent
            .lock()
            .unwrap()
            .push((to.to_string(), variables.clone()));
        Ok(())
    }
}

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
            serde_json::from_slice::<Value>(res.body()).unwrap_or(Value::Null),
        )
    }};
}

/// Point the sendgrid provider at `url` with a usable test API key.
async fn point_sendgrid_at(pool: &MySqlPool, url: &str) {
    sqlx::query(
        "UPDATE providers SET config = JSON_OBJECT('base_url', ?, 'api_key', 'test-key', 'from_email', 'no-reply@test.local') WHERE name = 'sendgrid'",
    )
    .bind(url)
    .execute(pool)
    .await
    .unwrap();
}

async fn message_row(pool: &MySqlPool, event_id: &str) -> Option<(String, Option<String>)> {
    sqlx::query_as("SELECT status, detail FROM messages WHERE event_id = ?")
        .bind(event_id)
        .fetch_optional(pool)
        .await
        .unwrap()
}

#[sqlx::test(migrator = "messenger_service::db::MIGRATOR")]
async fn sendgrid_request_body_contains_template_and_dynamic_data(pool: MySqlPool) {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v3/mail/send"))
        .respond_with(ResponseTemplate::new(202))
        .mount(&server)
        .await;
    point_sendgrid_at(&pool, &server.uri()).await;

    let api = build_router(state(pool.clone(), None));
    let (status, body) = post!(
        &api,
        "/send",
        json!({
            "event_type": "user.login",
            "to": "alice@example.com",
            "event_id": "11111111-1111-4111-8111-111111111111",
            "variables": { "username": "alice", "email": "alice@example.com" }
        })
    );
    assert_eq!(status, 200);
    assert_eq!(body["status"], "sent");
    assert_eq!(body["provider"], "sendgrid");

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    let sent: Value = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(sent["template_id"], "d-36dab063ce184e4180e716439b12ac9a");
    assert_eq!(
        sent["personalizations"][0]["to"][0]["email"],
        "alice@example.com"
    );
    assert_eq!(
        sent["personalizations"][0]["dynamic_template_data"]["username"],
        "alice"
    );

    let row = message_row(&pool, "11111111-1111-4111-8111-111111111111").await;
    assert_eq!(row.unwrap().0, "sent");
}

#[sqlx::test(migrator = "messenger_service::db::MIGRATOR")]
async fn sendgrid_500_marks_message_failed(pool: MySqlPool) {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v3/mail/send"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
    point_sendgrid_at(&pool, &server.uri()).await;

    let api = build_router(state(pool.clone(), None));
    let (status, body) = post!(
        &api,
        "/send",
        json!({
            "event_type": "user.login",
            "to": "bob@example.com",
            "event_id": "22222222-2222-4222-8222-222222222222",
            "variables": { "username": "bob", "email": "bob@example.com" }
        })
    );
    assert_eq!(status, 200);
    assert_eq!(body["status"], "failed");

    let (status, detail) = message_row(&pool, "22222222-2222-4222-8222-222222222222")
        .await
        .unwrap();
    assert_eq!(status, "failed");
    assert!(detail.unwrap().contains("500"));
}

#[sqlx::test(migrator = "messenger_service::db::MIGRATOR")]
async fn smtp_renders_subject_and_body(pool: MySqlPool) {
    // Route login to smtp for this (isolated) test DB.
    sqlx::query(
        "UPDATE event_routing SET provider_id = (SELECT id FROM providers WHERE name='smtp') WHERE event_type='user.login'",
    )
    .execute(&pool)
    .await
    .unwrap();

    let stub = Arc::new(StubTransport::new_ok());
    let api = build_router(state(pool.clone(), Some(stub.clone())));

    let (status, body) = post!(
        &api,
        "/send",
        json!({
            "event_type": "user.login",
            "to": "carol@example.com",
            "event_id": "33333333-3333-4333-8333-333333333333",
            "variables": { "username": "carol", "otp": "1234", "email": "carol@example.com" }
        })
    );
    assert_eq!(status, 200);
    assert_eq!(body["provider"], "smtp");
    assert_eq!(body["status"], "sent");

    let messages = stub.messages();
    assert_eq!(messages.len(), 1);
    let formatted = &messages[0].1;
    assert!(formatted.contains("Your verification code"), "{formatted}");
    assert!(
        formatted.contains("Hi carol, your verification code is 1234."),
        "{formatted}"
    );
}

#[sqlx::test(migrator = "messenger_service::db::MIGRATOR")]
async fn disabled_provider_short_circuits(pool: MySqlPool) {
    sqlx::query("UPDATE providers SET enabled = FALSE WHERE name = 'sendgrid'")
        .execute(&pool)
        .await
        .unwrap();

    let api = build_router(state(pool.clone(), None));
    let (status, body) = post!(
        &api,
        "/send",
        json!({
            "event_type": "user.login",
            "to": "dave@example.com",
            "event_id": "44444444-4444-4444-8444-444444444444",
            "variables": { "username": "dave", "email": "dave@example.com" }
        })
    );
    assert_eq!(status, 200);
    assert_eq!(body["status"], "failed");

    let (status, detail) = message_row(&pool, "44444444-4444-4444-8444-444444444444")
        .await
        .unwrap();
    assert_eq!(status, "failed");
    assert!(detail.unwrap().contains("disabled"));
}

#[sqlx::test(migrator = "messenger_service::db::MIGRATOR")]
async fn duplicate_event_id_is_sent_once(pool: MySqlPool) {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v3/mail/send"))
        .respond_with(ResponseTemplate::new(202))
        .mount(&server)
        .await;
    point_sendgrid_at(&pool, &server.uri()).await;

    let api = build_router(state(pool.clone(), None));
    let payload = json!({
        "event_type": "user.login",
        "to": "erin@example.com",
        "event_id": "55555555-5555-4555-8555-555555555555",
        "variables": { "username": "erin", "email": "erin@example.com" }
    });

    let (first_status, first_body) = post!(&api, "/send", payload.clone());
    assert_eq!(first_status, 200);
    assert_eq!(first_body["status"], "sent");

    let (second_status, second_body) = post!(&api, "/send", payload);
    assert_eq!(second_status, 200);
    assert_eq!(second_body["status"], "duplicate");

    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM messages WHERE event_id = '55555555-5555-4555-8555-555555555555'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 1);

    // Only one outbound HTTP call.
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}

#[sqlx::test(migrator = "messenger_service::db::MIGRATOR")]
async fn unknown_event_type_is_rejected(pool: MySqlPool) {
    let api = build_router(state(pool.clone(), None));
    let (status, body) = post!(
        &api,
        "/send",
        json!({
            "event_type": "user.unknown",
            "to": "frank@example.com",
            "variables": {}
        })
    );
    assert_eq!(status, 400);
    assert!(body["error"]
        .as_str()
        .unwrap()
        .contains("no provider routed"));
}

#[sqlx::test(migrator = "messenger_service::db::MIGRATOR")]
async fn health_is_ok(pool: MySqlPool) {
    let api = build_router(state(pool, None));
    let res = warp::test::request().path("/health").reply(&api).await;
    assert_eq!(res.status().as_u16(), 200);
}

#[sqlx::test(migrator = "messenger_service::db::MIGRATOR")]
async fn consumer_handler_uses_mock_sender_and_is_idempotent(pool: MySqlPool) {
    let mock = Arc::new(RecordingSender::default());
    let mut st = state(pool.clone(), None);
    st.registry.sender_override = Some(mock.clone());

    let event = events::Event::login("grace", "grace@example.com");

    // Consumer path: route → format → send → record.
    messenger_service::consumer::handle_event(&st, &event).await;
    assert_eq!(mock.sent.lock().unwrap().len(), 1);
    assert_eq!(mock.sent.lock().unwrap()[0].0, "grace@example.com");

    let (status, _) = message_row(&pool, &event.event_id).await.unwrap();
    assert_eq!(status, "sent");

    // Re-delivery of the same event is skipped (idempotent).
    messenger_service::consumer::handle_event(&st, &event).await;
    assert_eq!(mock.sent.lock().unwrap().len(), 1);
}
