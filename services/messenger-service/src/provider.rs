use async_trait::async_trait;
use std::collections::HashMap;

/// Provider delivery failure.
#[derive(Debug, Clone)]
pub struct SendError(pub String);

impl std::fmt::Display for SendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for SendError {}

/// Delivers one rendered message.
#[async_trait]
pub trait MessageSender: Send + Sync {
    async fn send(&self, to: &str, variables: &HashMap<String, String>) -> Result<(), SendError>;
}

/// SMTP transport seam (live `SmtpTransport` or `StubTransport` in tests).
pub trait EmailTransport: Send + Sync {
    fn send_email(&self, email: &lettre::Message) -> Result<(), SendError>;
}

impl EmailTransport for lettre::SmtpTransport {
    fn send_email(&self, email: &lettre::Message) -> Result<(), SendError> {
        use lettre::Transport;
        self.send(email)
            .map(|_| ())
            .map_err(|e| SendError(e.to_string()))
    }
}

impl EmailTransport for lettre::transport::stub::StubTransport {
    fn send_email(&self, email: &lettre::Message) -> Result<(), SendError> {
        use lettre::Transport;
        self.send(email)
            .map(|_| ())
            .map_err(|e| SendError(e.to_string()))
    }
}
