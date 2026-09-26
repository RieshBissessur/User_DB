use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use lettre::message::Mailbox;
use lettre::{Message, SmtpTransport};

use crate::db::{ProviderRow, SmtpFormatRow};

use crate::provider::{EmailTransport, MessageSender, SendError};
use crate::render::render;

/// SMTP provider; transport is injectable for tests.
pub struct SmtpSender {
    pub transport: Arc<dyn EmailTransport>,
    pub from: Mailbox,
    pub subject_template: String,
    pub body_template: String,
}

impl SmtpSender {
    /// Live plaintext transport. Host precedence: override → config → default.
    pub fn from_provider(
        provider: &ProviderRow,
        format: &SmtpFormatRow,
        default_from: &str,
        host_override: Option<String>,
        port_override: Option<u16>,
    ) -> Result<Self, SendError> {
        let cfg = &provider.config;
        let host = host_override
            .or_else(|| cfg.get("host").and_then(|v| v.as_str()).map(String::from))
            .unwrap_or_else(|| "localhost".to_string());
        let port = port_override
            .or_else(|| cfg.get("port").and_then(|v| v.as_u64()).map(|p| p as u16))
            .unwrap_or(1025);
        let from_email = cfg
            .get("from_email")
            .and_then(|v| v.as_str())
            .unwrap_or(default_from);

        let from: Mailbox = from_email
            .parse()
            .map_err(|e| SendError(format!("invalid from address: {e}")))?;
        tracing::info!(host = %host, port, "smtp transport ready");
        let transport = SmtpTransport::builder_dangerous(host).port(port).build();
        Ok(Self {
            transport: Arc::new(transport),
            from,
            subject_template: format.subject_template.clone(),
            body_template: format.body_template.clone(),
        })
    }

    /// Build with an injected transport (tests).
    pub fn with_transport(
        transport: Arc<dyn EmailTransport>,
        from_email: &str,
        format: &SmtpFormatRow,
    ) -> Result<Self, SendError> {
        let from: Mailbox = from_email
            .parse()
            .map_err(|e| SendError(format!("invalid from address: {e}")))?;
        Ok(Self {
            transport,
            from,
            subject_template: format.subject_template.clone(),
            body_template: format.body_template.clone(),
        })
    }
}

#[async_trait]
impl MessageSender for SmtpSender {
    async fn send(&self, to: &str, variables: &HashMap<String, String>) -> Result<(), SendError> {
        let subject =
            render(&self.subject_template, variables).map_err(|e| SendError(e.to_string()))?;
        let body = render(&self.body_template, variables).map_err(|e| SendError(e.to_string()))?;
        let to_mailbox: Mailbox = to
            .parse()
            .map_err(|e| SendError(format!("invalid to address: {e}")))?;

        let email = Message::builder()
            .from(self.from.clone())
            .to(to_mailbox)
            .subject(subject)
            .body(body)
            .map_err(|e| SendError(format!("failed to build email: {e}")))?;

        self.transport.send_email(&email)
    }
}
