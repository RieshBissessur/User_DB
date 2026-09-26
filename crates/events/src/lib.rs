//! Shared Kafka event contract for the `user-events` topic.
//!
//! ```json
//! { "schema_version": 1, "event_id": "<uuid v4>", "event_type": "user.login",
//!   "occurred_at": "2026-09-20T12:00:00Z",
//!   "username": "alice", "email": "alice@example.com",
//!   "payload": { "otp": "1234" } }
//! ```

#![cfg_attr(
    not(test),
    deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)
)]

use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Kafka topic carrying user lifecycle events.
pub const USER_EVENTS_TOPIC: &str = "user-events";

/// Consumer group used by the messenger service.
pub const MESSENGER_CONSUMER_GROUP: &str = "messenger-service";

/// Current event schema version. Consumers reject newer, unknown versions.
pub const SCHEMA_VERSION: u8 = 1;

fn default_schema_version() -> u8 {
    SCHEMA_VERSION
}

/// Deserialization-time rejection of events from a newer schema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsupportedSchema {
    pub got: u8,
    pub supported: u8,
}

impl std::fmt::Display for UnsupportedSchema {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "unsupported event schema_version {} (this build supports up to {})",
            self.got, self.supported
        )
    }
}

impl std::error::Error for UnsupportedSchema {}

/// Known event types. Deserialization rejects anything else.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventType {
    #[serde(rename = "user.login")]
    UserLogin,
    #[serde(rename = "user.password_reset_requested")]
    UserPasswordResetRequested,
}

impl EventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            EventType::UserLogin => "user.login",
            EventType::UserPasswordResetRequested => "user.password_reset_requested",
        }
    }
}

/// Event payload. `otp` is only present for password-reset events.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EventPayload {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub otp: Option<String>,
}

/// A single event published to the broker.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Event {
    /// Envelope schema version; absent in older messages → treated as current.
    #[serde(default = "default_schema_version")]
    pub schema_version: u8,
    pub event_id: String,
    pub event_type: EventType,
    pub occurred_at: String,
    pub username: String,
    pub email: String,
    #[serde(default)]
    pub payload: EventPayload,
}

impl Event {
    pub fn new(
        event_type: EventType,
        username: impl Into<String>,
        email: impl Into<String>,
        payload: EventPayload,
    ) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            event_id: Uuid::new_v4().to_string(),
            event_type,
            occurred_at: Utc::now().to_rfc3339(),
            username: username.into(),
            email: email.into(),
            payload,
        }
    }

    /// Reject events produced by a newer, unknown schema version.
    pub fn validate(&self) -> Result<(), UnsupportedSchema> {
        if self.schema_version > SCHEMA_VERSION {
            Err(UnsupportedSchema {
                got: self.schema_version,
                supported: SCHEMA_VERSION,
            })
        } else {
            Ok(())
        }
    }

    pub fn login(username: impl Into<String>, email: impl Into<String>) -> Self {
        Self::new(
            EventType::UserLogin,
            username,
            email,
            EventPayload::default(),
        )
    }

    pub fn password_reset(
        username: impl Into<String>,
        email: impl Into<String>,
        otp: impl Into<String>,
    ) -> Self {
        Self::new(
            EventType::UserPasswordResetRequested,
            username,
            email,
            EventPayload {
                otp: Some(otp.into()),
            },
        )
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    pub fn from_json(raw: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(raw)
    }
}

/// Build the template variables for rendering a provider message.
pub fn variables_for(event: &Event) -> std::collections::HashMap<String, String> {
    let mut vars = std::collections::HashMap::new();
    vars.insert("username".to_string(), event.username.clone());
    vars.insert("email".to_string(), event.email.clone());
    if let Some(otp) = &event.payload.otp {
        vars.insert("otp".to_string(), otp.clone());
    }
    vars
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        let event = Event::password_reset("alice", "alice@example.com", "1234");
        let raw = event.to_json().unwrap();
        let parsed = Event::from_json(&raw).unwrap();
        assert_eq!(parsed, event);
        assert_eq!(parsed.event_type, EventType::UserPasswordResetRequested);
        assert_eq!(parsed.payload.otp.as_deref(), Some("1234"));
    }

    #[test]
    fn login_event_has_empty_payload() {
        let event = Event::login("bob", "bob@example.com");
        let raw = event.to_json().unwrap();
        assert!(raw.contains("\"payload\":{}"), "raw={raw}");
    }

    #[test]
    fn unknown_event_type_is_rejected() {
        let raw = r#"{
            "event_id": "00000000-0000-4000-8000-000000000000",
            "event_type": "user.something_else",
            "occurred_at": "2026-09-20T12:00:00Z",
            "username": "alice",
            "email": "alice@example.com",
            "payload": {}
        }"#;
        assert!(Event::from_json(raw).is_err());
    }

    #[test]
    fn event_type_serializes_to_contract_strings() {
        assert_eq!(
            serde_json::to_string(&EventType::UserLogin).unwrap(),
            "\"user.login\""
        );
        assert_eq!(
            serde_json::to_string(&EventType::UserPasswordResetRequested).unwrap(),
            "\"user.password_reset_requested\""
        );
    }

    #[test]
    fn variables_include_otp_when_present() {
        let reset = Event::password_reset("alice", "alice@example.com", "9999");
        let vars = variables_for(&reset);
        assert_eq!(vars.get("otp").map(String::as_str), Some("9999"));

        let login = Event::login("alice", "alice@example.com");
        assert!(!variables_for(&login).contains_key("otp"));
    }

    #[test]
    fn new_events_carry_current_schema_version() {
        let event = Event::login("alice", "alice@example.com");
        assert_eq!(event.schema_version, SCHEMA_VERSION);
        assert!(event.validate().is_ok());
        assert!(event.to_json().unwrap().contains("\"schema_version\":1"));
    }

    #[test]
    fn missing_schema_version_defaults_to_current() {
        let raw = r#"{
            "event_id": "00000000-0000-4000-8000-000000000000",
            "event_type": "user.login",
            "occurred_at": "2026-09-20T12:00:00Z",
            "username": "alice",
            "email": "alice@example.com",
            "payload": {}
        }"#;
        let event = Event::from_json(raw).unwrap();
        assert_eq!(event.schema_version, SCHEMA_VERSION);
    }

    #[test]
    fn newer_schema_version_is_rejected() {
        let raw = r#"{
            "schema_version": 2,
            "event_id": "00000000-0000-4000-8000-000000000000",
            "event_type": "user.login",
            "occurred_at": "2026-09-20T12:00:00Z",
            "username": "alice",
            "email": "alice@example.com",
            "payload": {}
        }"#;
        let event = Event::from_json(raw).unwrap();
        assert_eq!(
            event.validate(),
            Err(UnsupportedSchema {
                got: 2,
                supported: SCHEMA_VERSION
            })
        );
    }
}
