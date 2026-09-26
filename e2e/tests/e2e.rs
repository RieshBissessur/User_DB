//! Cross-service e2e tests. Run against the compose stack:
//! `cargo test -p e2e -- --ignored --test-threads=1` (they share mailpit and
//! routing, so keep them single-threaded).

use e2e::{
    clear_mailpit, count_messages_to, extract_otp, messenger_database_url, messenger_url,
    unique_suffix, user_service_url, wait_for_message_text,
};
use serde_json::{json, Value};
use sqlx::MySqlPool;

const APP_VERSION: f32 = 0.1;

async fn post(client: &reqwest::Client, url: &str, body: Value) -> (u16, Value) {
    let response = client
        .post(url)
        .json(&body)
        .send()
        .await
        .unwrap_or_else(|e| panic!("request to {url} failed: {e}"));
    let status = response.status().as_u16();
    let value = response.json::<Value>().await.unwrap_or(Value::Null);
    (status, value)
}

async fn register(client: &reqwest::Client, username: &str, email: &str, password: &str) -> u16 {
    post(
        client,
        &format!("{}/register", user_service_url()),
        json!({ "username": username, "email": email, "password": password }),
    )
    .await
    .0
}

async fn login(
    client: &reqwest::Client,
    username: &str,
    password: &str,
    version: f32,
) -> (u16, Value) {
    post(
        client,
        &format!("{}/login", user_service_url()),
        json!({ "username": username, "password": password, "version": version }),
    )
    .await
}

async fn pool() -> MySqlPool {
    MySqlPool::connect(&messenger_database_url())
        .await
        .expect("failed to connect to messenger DB")
}

#[tokio::test]
#[ignore = "requires docker compose stack"]
async fn health_checks_both_services() {
    let client = reqwest::Client::new();
    for url in [
        format!("{}/health", user_service_url()),
        format!("{}/health", messenger_url()),
    ] {
        let response = client.get(&url).send().await.unwrap();
        assert_eq!(response.status().as_u16(), 200, "{url}");
    }
}

#[tokio::test]
#[ignore = "requires docker compose stack"]
async fn register_login_userdata_update() {
    let client = reqwest::Client::new();
    let s = unique_suffix();
    let username = format!("e2e{s}");
    let email = format!("e2e{s}@example.com");

    assert_eq!(register(&client, &username, &email, "pw123456").await, 200);

    let (status, body) = login(&client, &username, "pw123456", APP_VERSION).await;
    assert_eq!(status, 200);
    let session_key = body["session_key"].as_str().unwrap().to_string();

    let (status, body) = post(
        &client,
        &format!("{}/user_data", user_service_url()),
        json!({ "username": username, "session_key": session_key }),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(body["email"], email);

    let new_username = format!("{username}x");
    let (status, body) = post(
        &client,
        &format!("{}/update_user_data", user_service_url()),
        json!({
            "username": username,
            "new_username": new_username,
            "email": email,
            "avatar": "https://example.com/a.png",
            "session_key": session_key
        }),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(body["username"], new_username);

    let (status, body) = post(
        &client,
        &format!("{}/user_data", user_service_url()),
        json!({ "username": new_username, "session_key": session_key }),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(body["avatar"], "https://example.com/a.png");
}

#[tokio::test]
#[ignore = "requires docker compose stack"]
async fn last_logged_in_advances_on_second_login() {
    let client = reqwest::Client::new();
    let db = pool().await;
    let s = unique_suffix();
    let username = format!("e2e{s}");
    let email = format!("e2e{s}@example.com");

    assert_eq!(register(&client, &username, &email, "pw").await, 200);
    login(&client, &username, "pw", APP_VERSION).await;

    let first: Option<chrono::NaiveDateTime> =
        sqlx::query_scalar("SELECT last_logged_in FROM user_service.users WHERE username = ?")
            .bind(&username)
            .fetch_one(&db)
            .await
            .unwrap();
    assert!(first.is_some());

    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    login(&client, &username, "pw", APP_VERSION).await;

    let second: Option<chrono::NaiveDateTime> =
        sqlx::query_scalar("SELECT last_logged_in FROM user_service.users WHERE username = ?")
            .bind(&username)
            .fetch_one(&db)
            .await
            .unwrap();
    assert!(second > first, "last_logged_in should advance");
}

#[tokio::test]
#[ignore = "requires docker compose stack"]
async fn wrong_password_and_version_gate() {
    let client = reqwest::Client::new();
    let s = unique_suffix();
    let username = format!("e2e{s}");
    let email = format!("e2e{s}@example.com");
    assert_eq!(register(&client, &username, &email, "right").await, 200);

    let (status, _) = login(&client, &username, "wrong", APP_VERSION).await;
    assert_eq!(status, 400);

    let (status, _) = login(&client, &username, "right", 0.0).await;
    assert_eq!(status, 400);
}

#[tokio::test]
#[ignore = "requires docker compose stack"]
async fn password_reset_via_real_email_is_single_use() {
    let client = reqwest::Client::new();
    clear_mailpit(&client).await;
    let s = unique_suffix();
    let username = format!("e2e{s}");
    let email = format!("e2e{s}@example.com");
    assert_eq!(register(&client, &username, &email, "oldpass").await, 200);

    let (status, _) = post(
        &client,
        &format!("{}/reset_request", user_service_url()),
        json!({ "email": email }),
    )
    .await;
    assert_eq!(status, 200);

    // OTP arrives by real email (messenger → SMTP → mailpit).
    let text = wait_for_message_text(&client, &email, 30).await;
    let otp = extract_otp(&text).expect("no OTP in email");
    assert_eq!(otp.len(), 4);

    let (status, body) = post(
        &client,
        &format!("{}/check_otp", user_service_url()),
        json!({ "otp": otp, "email": email, "password": "newpass" }),
    )
    .await;
    assert_eq!(status, 200);
    assert!(body.as_str().unwrap().contains("OTP match and valid"));

    // New password works, old one does not.
    assert_eq!(
        login(&client, &username, "newpass", APP_VERSION).await.0,
        200
    );
    assert_eq!(
        login(&client, &username, "oldpass", APP_VERSION).await.0,
        400
    );

    // Single-use: the consumed OTP is rejected.
    let (status, body) = post(
        &client,
        &format!("{}/check_otp", user_service_url()),
        json!({ "otp": otp, "email": email, "password": "again" }),
    )
    .await;
    assert_eq!(status, 200);
    assert!(body.as_str().unwrap().contains("OTP invalid or expired"));
}

#[tokio::test]
#[ignore = "requires docker compose stack"]
async fn duplicate_event_id_sends_exactly_one_email() {
    let client = reqwest::Client::new();
    let db = pool().await;
    clear_mailpit(&client).await;

    let s = unique_suffix();
    let recipient = format!("dup{s}@example.com");
    let event_id = uuid::Uuid::new_v4().to_string();
    let body = json!({
        "event_type": "user.password_reset_requested",
        "to": recipient,
        "event_id": event_id,
        "variables": { "username": format!("dup{s}"), "otp": "4242", "email": recipient }
    });

    let (first_status, first) =
        post(&client, &format!("{}/send", messenger_url()), body.clone()).await;
    assert_eq!(first_status, 200);
    assert_eq!(first["status"], "sent");

    let (_, second) = post(&client, &format!("{}/send", messenger_url()), body).await;
    assert_eq!(second["status"], "duplicate");

    // Exactly one email and exactly one messages row.
    assert_eq!(count_messages_to(&client, &recipient).await, 1);
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM messages WHERE event_id = ?")
        .bind(&event_id)
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
#[ignore = "requires docker compose stack"]
async fn routing_switch_changes_provider() {
    let client = reqwest::Client::new();
    let db = pool().await;
    const EVENT: &str = "user.password_reset_requested";

    let send_via = |label: &str| {
        let recipient = format!("route{label}{}@example.com", unique_suffix());
        let event_id = uuid::Uuid::new_v4().to_string();
        let body = json!({
            "event_type": EVENT,
            "to": recipient,
            "event_id": event_id,
            "variables": { "username": "router", "otp": "1234", "email": recipient }
        });
        (event_id, body)
    };

    // Default routing → smtp.
    let (smtp_event, smtp_body) = send_via("smtp");
    post(&client, &format!("{}/send", messenger_url()), smtp_body).await;

    // Flip → sendgrid.
    sqlx::query(
        "UPDATE event_routing SET provider_id = (SELECT id FROM providers WHERE name='sendgrid') WHERE event_type = ?",
    )
    .bind(EVENT)
    .execute(&db)
    .await
    .unwrap();
    let (sendgrid_event, sendgrid_body) = send_via("sg");
    post(&client, &format!("{}/send", messenger_url()), sendgrid_body).await;

    // Flip back → smtp.
    sqlx::query(
        "UPDATE event_routing SET provider_id = (SELECT id FROM providers WHERE name='smtp') WHERE event_type = ?",
    )
    .bind(EVENT)
    .execute(&db)
    .await
    .unwrap();

    let provider_for = |event_id: String| {
        let db = db.clone();
        async move {
            let name: String = sqlx::query_scalar(
                "SELECT p.name FROM messages m JOIN providers p ON p.id = m.provider_id WHERE m.event_id = ?",
            )
            .bind(event_id)
            .fetch_one(&db)
            .await
            .unwrap();
            name
        }
    };

    assert_eq!(provider_for(smtp_event).await, "smtp");
    assert_eq!(provider_for(sendgrid_event).await, "sendgrid");
}
