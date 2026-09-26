#![cfg_attr(
    not(test),
    deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)
)]

pub mod config;
pub mod db;
pub mod models;
pub mod normalize;
pub mod otp;
pub mod password;
pub mod producer;
pub mod routes;
pub mod session_key;
pub mod version;
pub mod workers;

use routes::{build_router, AppState};

/// Application version used by the login version gate (preserved from legacy).
pub const APP_VERSION: f32 = 0.1;

/// Bootstrap: load env, connect to MySQL, run migrations, serve HTTP.
pub async fn run() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    dotenvy::dotenv().ok();
    init_tracing();

    let config = config::Config::from_env()?;

    let pool = db_core::connect(&config.database_url).await?;
    db_core::run_migrations(&pool, &db::MIGRATOR).await?;

    let state = AppState {
        pool,
        otp_ttl_minutes: config.otp_ttl_minutes,
        session_ttl_minutes: config.session_ttl_minutes,
        app_version: APP_VERSION,
        producer: Some(producer::Producer::new(&config.kafka_brokers)?),
    };

    // Background workers: sweep expired sessions, publish the outbox to Kafka.
    workers::start_session_cleanup(state.pool.clone(), config.session_cleanup_interval_seconds);
    workers::start_outbox_publisher(state.clone(), config.outbox_poll_ms);

    let routes = build_router(state);
    tracing::info!(port = config.service_port, "user-service listening");
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
