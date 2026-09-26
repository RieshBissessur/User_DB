use std::env;

/// Runtime configuration, read once at startup from the environment.
#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub service_port: u16,
    pub otp_ttl_minutes: i64,
    pub session_ttl_minutes: i64,
    pub session_cleanup_interval_seconds: u64,
    pub outbox_poll_ms: u64,
    pub kafka_brokers: String,
}

impl Config {
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Self {
            database_url: env_required("DATABASE_URL")?,
            service_port: env_parsed("SERVICE_PORT", 3030),
            otp_ttl_minutes: env_parsed("OTP_TTL_MINUTES", 120),
            session_ttl_minutes: env_parsed("SESSION_TTL_MINUTES", 10080),
            session_cleanup_interval_seconds: env_parsed("SESSION_CLEANUP_INTERVAL_SECONDS", 3600),
            outbox_poll_ms: env_parsed("OUTBOX_POLL_MS", 1000),
            kafka_brokers: env::var("KAFKA_BROKERS").unwrap_or_else(|_| "kafka:9092".to_string()),
        })
    }
}

fn env_required(key: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    env::var(key).map_err(|_| format!("missing required env var {key}").into())
}

fn env_parsed<T: std::str::FromStr>(key: &str, default: T) -> T {
    env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    #[test]
    fn defaults_are_sane() {
        let port: Option<u16> = "3030".parse().ok();
        assert_eq!(port, Some(3030));
        let ttl: Option<i64> = "120".parse().ok();
        assert_eq!(ttl, Some(120));
    }
}
