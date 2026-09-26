use std::collections::HashMap;

use async_trait::async_trait;
use serde::Serialize;

use crate::db::{ProviderRow, SendgridFormatRow};

use crate::provider::{MessageSender, SendError};

#[derive(Debug, Serialize)]
struct SendGridEmail {
    personalizations: Vec<Personalization>,
    from: EmailAddress,
    template_id: String,
}

#[derive(Debug, Serialize)]
struct Personalization {
    to: Vec<EmailAddress>,
    dynamic_template_data: HashMap<String, String>,
}

#[derive(Debug, Serialize)]
struct EmailAddress {
    email: String,
}

/// SendGrid v3 HTTP provider; endpoints/keys come from the provider row.
pub struct SendgridSender {
    pub client: reqwest::Client,
    pub base_url: String,
    pub api_key: String,
    pub from_email: String,
    pub template_id: String,
    /// placeholder (template key) -> variable name
    pub dynamic_fields: HashMap<String, String>,
}

impl SendgridSender {
    pub fn from_provider(
        provider: &ProviderRow,
        format: &SendgridFormatRow,
        default_base_url: &str,
    ) -> Result<Self, SendError> {
        let cfg = &provider.config;

        let base_url = cfg
            .get("base_url")
            .and_then(|v| v.as_str())
            .unwrap_or(default_base_url)
            .to_string();

        let from_email = cfg
            .get("from_email")
            .and_then(|v| v.as_str())
            .unwrap_or("no-reply@pattern-shop.local")
            .to_string();

        // Explicit key (tests) or the named env var.
        let api_key = if let Some(key) = cfg.get("api_key").and_then(|v| v.as_str()) {
            key.to_string()
        } else if let Some(env_name) = cfg.get("api_key_env").and_then(|v| v.as_str()) {
            std::env::var(env_name).unwrap_or_default()
        } else {
            String::new()
        };

        let dynamic_fields = serde_json::from_value(format.dynamic_fields.clone())
            .map_err(|e| SendError(format!("invalid dynamic_fields: {e}")))?;

        Ok(Self {
            client: reqwest::Client::new(),
            base_url,
            api_key,
            from_email,
            template_id: format.template_id.clone(),
            dynamic_fields,
        })
    }

    fn disabled(&self) -> bool {
        let key = self.api_key.trim();
        key.is_empty() || key == "API_KEY"
    }
}

#[async_trait]
impl MessageSender for SendgridSender {
    async fn send(&self, to: &str, variables: &HashMap<String, String>) -> Result<(), SendError> {
        if self.disabled() {
            return Err(SendError("sendgrid api key not configured".to_string()));
        }

        let mut dynamic_template_data = HashMap::new();
        for (placeholder, variable) in &self.dynamic_fields {
            let value = variables.get(variable).cloned().unwrap_or_default();
            dynamic_template_data.insert(placeholder.clone(), value);
        }

        let payload = SendGridEmail {
            personalizations: vec![Personalization {
                to: vec![EmailAddress {
                    email: to.to_string(),
                }],
                dynamic_template_data,
            }],
            from: EmailAddress {
                email: self.from_email.clone(),
            },
            template_id: self.template_id.clone(),
        };

        let url = format!("{}/v3/mail/send", self.base_url.trim_end_matches('/'));
        let response = self
            .client
            .post(url)
            .bearer_auth(&self.api_key)
            .json(&payload)
            .send()
            .await
            .map_err(|e| SendError(e.to_string()))?;

        if !response.status().is_success() {
            return Err(SendError(format!(
                "sendgrid returned {}",
                response.status()
            )));
        }
        Ok(())
    }
}
