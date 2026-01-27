//! Error types for the gateway.

use thiserror::Error;

/// Gateway error types
#[derive(Error, Debug)]
pub enum GatewayError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Proxy error: {0}")]
    Proxy(String),

    #[error("Route not found: {0}")]
    RouteNotFound(String),

    #[error("Upstream error: {0}")]
    Upstream(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl actix_web::ResponseError for GatewayError {
    fn error_response(&self) -> actix_web::HttpResponse {
        use actix_web::http::StatusCode;

        let (status, title) = match self {
            GatewayError::RouteNotFound(_) => (StatusCode::NOT_FOUND, "Not Found"),
            GatewayError::Upstream(_) | GatewayError::Proxy(_) => {
                (StatusCode::BAD_GATEWAY, "Bad Gateway")
            }
            _ => (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error"),
        };

        let html = crate::web::templates::error_page(status.as_u16(), title);

        actix_web::HttpResponse::build(status)
            .content_type("text/html; charset=utf-8")
            .body(html)
    }
}
