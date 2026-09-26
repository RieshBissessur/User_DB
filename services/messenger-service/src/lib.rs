#![cfg_attr(
    not(test),
    deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)
)]

pub mod config;
pub mod consumer;
pub mod db;
pub mod models;
pub mod provider;
pub mod providers;
pub mod render;
pub mod routes;
pub mod service;

pub use provider::{MessageSender, SendError};

use routes::build_router;
use service::{AppState, Registry};

/// Bootstrap: load env, connect to MySQL, run migrations, serve HTTP.
pub async fn run() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    dotenvy::dotenv().ok();
    init_tracing();

    let config = config::Config::from_env()?;

    let pool = db_core::connect(&config.database_url).await?;
    db_core::run_migrations(&pool, &db::MIGRATOR).await?;

    let state = AppState {
        pool,
        registry: Registry::live(
            config.sendgrid_base_url.clone(),
            config.smtp_from.clone(),
            config.smtp_host.clone(),
            config.smtp_port,
        ),
    };

    consumer::start_consumer(state.clone(), &config.kafka_brokers);

    let routes = build_router(state);
    tracing::info!(port = config.service_port, "messenger-service listening");
    warp::serve(routes)
        .run(([0, 0, 0, 0], config.service_port))
        .await;
    Ok(())
}

fn init_tracing() {
    use tracing_subscriber::EnvFilter;
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
}
