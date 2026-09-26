use std::collections::HashMap;
use std::sync::Arc;

use sqlx::MySqlPool;

use crate::db as repo;
use crate::db::ProviderRow;

use crate::models::SendResponse;
use crate::provider::{EmailTransport, MessageSender};
use crate::providers::sendgrid::SendgridSender;
use crate::providers::smtp::SmtpSender;
use crate::render::RenderError;
use crate::SendError;

/// Dependencies for building senders; tests inject stubs.
#[derive(Clone)]
pub struct Registry {
    pub sendgrid_base_url: String,
    pub smtp_from: String,
    /// Host-run overrides for the SMTP relay (env `SMTP_HOST`/`SMTP_PORT`).
    /// When absent, the provider `config` JSON decides (compose seeds `mailpit`).
    pub smtp_host_override: Option<String>,
    pub smtp_port_override: Option<u16>,
    pub smtp_transport_override: Option<Arc<dyn EmailTransport>>,
    /// When set, every provider resolves to this sender (tests / mocks).
    pub sender_override: Option<Arc<dyn MessageSender>>,
}

impl Registry {
    pub fn live(
        sendgrid_base_url: impl Into<String>,
        smtp_from: impl Into<String>,
        smtp_host_override: Option<String>,
        smtp_port_override: Option<u16>,
    ) -> Self {
        Self {
            sendgrid_base_url: sendgrid_base_url.into(),
            smtp_from: smtp_from.into(),
            smtp_host_override,
            smtp_port_override,
            smtp_transport_override: None,
            sender_override: None,
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub pool: MySqlPool,
    pub registry: Registry,
}

#[derive(Debug)]
pub enum ServiceError {
    Db(sqlx::Error),
    NoRoute(String),
    Provider(String),
}

impl std::fmt::Display for ServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServiceError::Db(e) => write!(f, "database error: {e}"),
            ServiceError::NoRoute(event_type) => write!(f, "no provider routed for {event_type}"),
            ServiceError::Provider(e) => write!(f, "provider error: {e}"),
        }
    }
}

impl std::error::Error for ServiceError {}

impl From<sqlx::Error> for ServiceError {
    fn from(e: sqlx::Error) -> Self {
        ServiceError::Db(e)
    }
}

impl From<SendError> for ServiceError {
    fn from(e: SendError) -> Self {
        ServiceError::Provider(e.0)
    }
}

impl From<RenderError> for ServiceError {
    fn from(e: RenderError) -> Self {
        ServiceError::Provider(e.to_string())
    }
}

/// Route → send → record. Shared by `POST /send` and the consumer.
pub async fn process_message(
    state: &AppState,
    event_id: &str,
    event_type: &str,
    to: &str,
    variables: &HashMap<String, String>,
) -> Result<SendResponse, ServiceError> {
    // Idempotency: a known event_id is never delivered twice.
    if repo::message_exists(&state.pool, event_id).await? {
        tracing::info!(event_id, "duplicate event; skipping");
        return Ok(SendResponse {
            event_id: event_id.to_string(),
            status: "duplicate".to_string(),
            provider: None,
            detail: None,
        });
    }

    let provider = repo::find_provider_for_event(&state.pool, event_type)
        .await?
        .ok_or_else(|| ServiceError::NoRoute(event_type.to_string()))?;

    if !provider.enabled {
        let detail = format!("provider '{}' disabled", provider.name);
        repo::insert_message(
            &state.pool,
            event_id,
            event_type,
            provider.id,
            "failed",
            Some(&detail),
        )
        .await?;
        return Ok(SendResponse {
            event_id: event_id.to_string(),
            status: "failed".to_string(),
            provider: Some(provider.name),
            detail: Some(detail),
        });
    }

    let sender = build_sender(state, &provider).await?;
    let send_result = sender.send(to, variables).await;

    let (status, detail) = match &send_result {
        Ok(()) => ("sent", None),
        Err(e) => ("failed", Some(e.to_string())),
    };
    repo::insert_message(
        &state.pool,
        event_id,
        event_type,
        provider.id,
        status,
        detail.as_deref(),
    )
    .await?;

    if let Err(e) = &send_result {
        tracing::warn!(event_id, provider = %provider.name, error = %e, "delivery failed");
    } else {
        tracing::info!(event_id, provider = %provider.name, "delivered");
    }

    Ok(SendResponse {
        event_id: event_id.to_string(),
        status: status.to_string(),
        provider: Some(provider.name),
        detail,
    })
}

async fn build_sender(
    state: &AppState,
    provider: &ProviderRow,
) -> Result<Arc<dyn MessageSender>, ServiceError> {
    if let Some(sender) = &state.registry.sender_override {
        return Ok(sender.clone());
    }
    match provider.name.as_str() {
        "sendgrid" => {
            let format = repo::find_sendgrid_format(&state.pool, provider.id)
                .await?
                .ok_or_else(|| ServiceError::Provider("sendgrid format missing".to_string()))?;
            let sender = SendgridSender::from_provider(
                provider,
                &format,
                &state.registry.sendgrid_base_url,
            )?;
            Ok(Arc::new(sender))
        }
        "smtp" => {
            let format = repo::find_smtp_format(&state.pool, provider.id)
                .await?
                .ok_or_else(|| ServiceError::Provider("smtp format missing".to_string()))?;
            if let Some(transport) = &state.registry.smtp_transport_override {
                let from = provider
                    .config
                    .get("from_email")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&state.registry.smtp_from);
                let sender = SmtpSender::with_transport(transport.clone(), from, &format)?;
                Ok(Arc::new(sender))
            } else {
                let sender = SmtpSender::from_provider(
                    provider,
                    &format,
                    &state.registry.smtp_from,
                    state.registry.smtp_host_override.clone(),
                    state.registry.smtp_port_override,
                )?;
                Ok(Arc::new(sender))
            }
        }
        other => Err(ServiceError::Provider(format!(
            "unknown provider '{other}'"
        ))),
    }
}
