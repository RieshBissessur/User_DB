//! Shared helpers for the cross-service e2e tests.

use serde_json::Value;

pub fn user_service_url() -> String {
    std::env::var("USER_SERVICE_URL").unwrap_or_else(|_| "http://localhost:3030".into())
}

pub fn messenger_url() -> String {
    std::env::var("MESSENGER_URL").unwrap_or_else(|_| "http://localhost:3031".into())
}

pub fn mailpit_url() -> String {
    std::env::var("MAILPIT_URL").unwrap_or_else(|_| "http://localhost:8025".into())
}

pub fn messenger_database_url() -> String {
    if let Ok(url) = std::env::var("E2E_DATABASE_URL") {
        return url;
    }
    // Fall back to the local root password from the environment (.env / make).
    let password = std::env::var("MYSQL_ROOT_PASSWORD")
        .expect("set E2E_DATABASE_URL or MYSQL_ROOT_PASSWORD (run `make env`)");
    format!("mysql://root:{password}@127.0.0.1:3306/messenger_service")
}

pub fn unique_suffix() -> String {
    uuid::Uuid::new_v4().simple().to_string()[..8].to_string()
}

/// Clear all mailpit messages.
pub async fn clear_mailpit(client: &reqwest::Client) {
    let _ = client
        .delete(format!("{}/api/v1/messages", mailpit_url()))
        .send()
        .await;
}

/// All mailpit messages addressed to `to` (case-insensitive).
async fn messages_to(client: &reqwest::Client, to: &str) -> Vec<Value> {
    let response = client
        .get(format!("{}/api/v1/messages", mailpit_url()))
        .send()
        .await
        .expect("mailpit list request failed");
    let body: Value = response.json().await.expect("mailpit list was not JSON");
    let to = to.to_lowercase();

    body["messages"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|message| {
            message["To"]
                .as_array()
                .map(|recipients| {
                    recipients.iter().any(|r| {
                        r["Address"]
                            .as_str()
                            .map(|a| a.to_lowercase() == to)
                            .unwrap_or(false)
                    })
                })
                .unwrap_or(false)
        })
        .collect()
}

/// Fetch the plain-text body of a mailpit message by ID.
async fn message_text(client: &reqwest::Client, id: &str) -> String {
    let response = client
        .get(format!("{}/api/v1/message/{id}", mailpit_url()))
        .send()
        .await
        .expect("mailpit message request failed");
    let body: Value = response.json().await.expect("mailpit message was not JSON");
    body["Text"]
        .as_str()
        .or_else(|| body["HTML"].as_str())
        .unwrap_or_default()
        .to_string()
}

/// Wait until at least one message addressed to `to` exists, and return its body.
pub async fn wait_for_message_text(
    client: &reqwest::Client,
    to: &str,
    timeout_secs: u64,
) -> String {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);
    loop {
        let messages = messages_to(client, to).await;
        if let Some(id) = messages.first().and_then(|m| m["ID"].as_str()) {
            let text = message_text(client, id).await;
            if !text.is_empty() {
                return text;
            }
        }
        if std::time::Instant::now() >= deadline {
            panic!("timed out waiting for an email to {to}");
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
}

/// Count emails addressed to `to`.
pub async fn count_messages_to(client: &reqwest::Client, to: &str) -> usize {
    messages_to(client, to).await.len()
}

/// Extract a 4-digit OTP from a message body. Prefers the text after "code is ",
/// then falls back to the first standalone 4-digit group.
pub fn extract_otp(text: &str) -> Option<String> {
    if let Some(idx) = text.find("code is ") {
        let rest = &text[idx + "code is ".len()..];
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        if digits.len() >= 4 {
            return Some(digits[..4].to_string());
        }
    }
    let bytes = text.as_bytes();
    let mut run = String::new();
    for &b in bytes {
        if b.is_ascii_digit() {
            run.push(b as char);
            if run.len() == 4 {
                return Some(run);
            }
        } else {
            run.clear();
        }
    }
    None
}
