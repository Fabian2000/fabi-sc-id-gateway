//! Web UI module for the gateway admin interface.

mod api;
mod handlers;
pub mod templates;

use actix_files::Files;
use actix_web::{middleware, web, App, HttpServer};
use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::info;

use crate::db::DbPool;
use crate::proxy::{proxy_handler, ProxyState};

/// Run the gateway server.
pub async fn run_server(
    pool: DbPool,
    bind_addr: &str,
    tls_config: Option<(PathBuf, PathBuf)>,
) -> Result<()> {
    let proxy_state = Arc::new(ProxyState::new(pool.clone()));

    // Load initial routes
    proxy_state.reload_routes().await?;

    let proxy_state_data = web::Data::new(proxy_state.clone());
    let pool_data = web::Data::new(pool);

    let server = HttpServer::new(move || {
        App::new()
            .app_data(proxy_state_data.clone())
            .app_data(pool_data.clone())
            // Increase payload size limit to 100MB for WebSocket messages
            .app_data(web::PayloadConfig::new(100 * 1024 * 1024))
            .wrap(middleware::Logger::default())
            .wrap(middleware::Compress::default())
            // Admin UI routes
            .service(
                web::scope("/_admin")
                    .route("", web::get().to(handlers::admin_dashboard))
                    .route("/", web::get().to(handlers::admin_dashboard))
                    .route("/login", web::get().to(handlers::login_page))
                    .route("/callback", web::get().to(handlers::auth_callback))
                    .route("/callback-session", web::get().to(handlers::auth_callback_session))
                    .route("/logout", web::post().to(handlers::logout))
                    .route("/setup", web::get().to(handlers::setup_page))
                    .route("/setup", web::post().to(handlers::setup_submit))
                    .route("/routes", web::get().to(handlers::routes_page))
                    .route("/settings", web::get().to(handlers::settings_page))
                    // API endpoints
                    .route("/api/routes", web::get().to(api::list_routes))
                    .route("/api/routes", web::post().to(api::create_route))
                    .route("/api/routes/{id}", web::put().to(api::update_route))
                    .route("/api/routes/{id}", web::delete().to(api::delete_route))
                    .route("/api/settings/id", web::get().to(api::get_id_config))
                    .route("/api/settings/id", web::put().to(api::update_id_config))
                    .route("/api/reload", web::post().to(api::reload_routes)),
            )
            // Static files for admin UI
            .service(Files::new("/_static", "./static").show_files_listing())
            // Proxy all other requests
            .default_service(web::route().to(proxy_handler))
    });

    if let Some((cert_path, key_path)) = tls_config {
        // Load TLS config using rustls-pki-types
        use rustls_pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject};

        let certs: Vec<CertificateDer<'static>> = CertificateDer::pem_file_iter(&cert_path)?
            .collect::<Result<Vec<_>, _>>()?;
        let key = PrivateKeyDer::from_pem_file(&key_path)?;

        let config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(certs, key)?;

        info!("Starting HTTPS server on {}", bind_addr);
        server
            .bind_rustls_0_23(bind_addr, config)?
            .run()
            .await?;
    } else {
        info!("Starting HTTP server on {}", bind_addr);
        server.bind(bind_addr)?.run().await?;
    }

    Ok(())
}
