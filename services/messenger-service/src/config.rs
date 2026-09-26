use std::env;

/// Runtime configuration for the messenger service.
#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub service_port: u16,
    pub kafka_brokers: String,
    pub sendgrid_base_url: String,
    pub smtp_from: String,
    pub smtp_host: Option<String>,
    pub smtp_port: Option<u16>,
}

impl Config {
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Self {
            database_url: required("MESSENGER_DATABASE_URL")?,
            service_port: env::var("MESSENGER_PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(3031),
            kafka_brokers: env::var("KAFKA_BROKERS").unwrap_or_else(|_| "kafka:9092".to_string()),
            sendgrid_base_url: env::var("SENDGRID_BASE_URL")
                .unwrap_or_else(|_| "https://api.sendgrid.com".to_string()),
            smtp_from: env::var("SMTP_FROM")
                .unwrap_or_else(|_| "no-reply@pattern-shop.local".to_string()),
            smtp_host: env::var("SMTP_HOST").ok(),
            smtp_port: env::var("SMTP_PORT").ok().and_then(|v| v.parse().ok()),
        })
    }
}

fn required(key: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    env::var(key).map_err(|_| format!("missing required env var {key}").into())
}
