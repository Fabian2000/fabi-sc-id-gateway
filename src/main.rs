//! Fabi-SC ID Gateway
//!
//! A self-hosted reverse proxy with Fabi-SC ID authentication.

mod auth;
mod config;
mod db;
mod error;
mod proxy;
mod web;

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use tracing::{info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Fabi-SC ID Gateway - Self-hosted reverse proxy with authentication
#[derive(Parser, Debug)]
#[command(name = "fabi-sc-id-gateway")]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Port to listen on
    #[arg(short, long, default_value = "8080")]
    pub port: u16,

    /// Host to bind to
    #[arg(long, default_value = "0.0.0.0")]
    pub host: String,

    /// Path to SQLite database file
    #[arg(short, long, default_value = "gateway.db")]
    pub database: PathBuf,

    /// Path to TLS certificate file (PEM format)
    #[arg(long)]
    pub tls_cert: Option<PathBuf>,

    /// Path to TLS private key file (PEM format)
    #[arg(long)]
    pub tls_key: Option<PathBuf>,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, default_value = "info")]
    pub log_level: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize logging
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(&args.log_level));

    tracing_subscriber::registry()
        .with(env_filter)
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!(
        "Starting Fabi-SC ID Gateway v{}",
        env!("CARGO_PKG_VERSION")
    );

    // Validate TLS configuration
    let tls_config = match (&args.tls_cert, &args.tls_key) {
        (Some(cert), Some(key)) => {
            info!("TLS enabled with cert: {:?}, key: {:?}", cert, key);
            Some((cert.clone(), key.clone()))
        }
        (Some(_), None) | (None, Some(_)) => {
            anyhow::bail!("Both --tls-cert and --tls-key must be provided for TLS");
        }
        (None, None) => {
            warn!("TLS disabled - running in plain HTTP mode");
            None
        }
    };

    // Initialize database
    let db_pool = db::init(&args.database).await?;

    // Check if this is first run (migration mode)
    if db::is_first_run(&db_pool).await? {
        info!("First run detected - entering setup mode");
    }

    // Start the server
    let bind_addr = format!("{}:{}", args.host, args.port);
    info!("Listening on {}", bind_addr);

    web::run_server(db_pool, &bind_addr, tls_config).await?;

    Ok(())
}
