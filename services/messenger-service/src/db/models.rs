use sqlx::FromRow;

/// Row of the `providers` table.
#[derive(Debug, Clone, FromRow)]
pub struct ProviderRow {
    pub id: u64,
    pub name: String,
    pub enabled: bool,
    /// Non-secret provider config; secrets are referenced by env-var name.
    pub config: serde_json::Value,
}

/// Row of the `format_sendgrid` table.
#[derive(Debug, Clone, FromRow)]
pub struct SendgridFormatRow {
    pub provider_id: u64,
    pub template_id: String,
    /// Placeholder (template key) -> variable name.
    pub dynamic_fields: serde_json::Value,
}

/// Row of the `format_smtp` table.
#[derive(Debug, Clone, FromRow)]
pub struct SmtpFormatRow {
    pub provider_id: u64,
    pub subject_template: String,
    pub body_template: String,
}
