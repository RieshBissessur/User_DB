use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Request body for `POST /send`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SendRequest {
    pub event_type: String,
    pub to: String,
    #[serde(default)]
    pub variables: HashMap<String, String>,
    /// Optional idempotency key; generated when omitted.
    #[serde(default)]
    pub event_id: Option<String>,
}

/// Response body for `POST /send`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendResponse {
    pub event_id: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// Rejection used to surface handler errors as 4xx/5xx.
#[derive(Debug)]
pub struct MessageRejection(pub String);

impl warp::reject::Reject for MessageRejection {}
