//! Reverse proxy module for HTTP, WebSocket, and HTTP/3 support.

mod handler;
mod websocket;

pub use handler::*;
#[allow(unused_imports)]
pub use websocket::*;

use actix_web::{http::header, HttpRequest};

use crate::config::Route;

/// Resolve the host a request was addressed to, from the real `Host` header.
///
/// Never use `connection_info().host()` for routing or authorization: actix-web
/// resolves it as `Forwarded` -> `X-Forwarded-Host` -> `Host`, so a client can
/// override it and impersonate any configured host.
pub fn request_host(req: &HttpRequest) -> String {
    req.headers()
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .split(':')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
}

/// Best-effort client IP for audit logging.
///
/// mod_proxy appends the peer it actually saw to `X-Forwarded-For`, so the last
/// entry is the only one a client cannot forge.
pub fn client_ip(req: &HttpRequest) -> String {
    req.headers()
        .get(header::X_FORWARDED_FOR)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.rsplit(',').next())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| req.peer_addr().map(|a| a.ip().to_string()))
        .unwrap_or_else(|| "unknown".to_string())
}

/// Extract the bare hostname from an origin or URL (no scheme, port, or path).
pub fn host_from_origin(origin: &str) -> Option<String> {
    let rest = origin.trim();
    let rest = rest.split_once("://").map_or(rest, |(_, r)| r);
    let rest = rest.split('/').next().unwrap_or(rest);
    let host = rest.split(':').next().unwrap_or(rest).trim();

    (!host.is_empty()).then(|| host.to_ascii_lowercase())
}

/// Find a matching route for the given host.
pub fn find_route<'a>(routes: &'a [Route], host: &str) -> Option<&'a Route> {
    // Strip port from host if present
    let host_without_port = host.split(':').next().unwrap_or(host);
    routes
        .iter()
        .find(|r| r.enabled && r.host == host_without_port)
}

/// Build the upstream URL for a request.
pub fn build_upstream_url(route: &Route, path: &str, query: Option<&str>) -> String {
    let mut url = format!("{}{}", route.upstream_url.trim_end_matches('/'), path);

    if let Some(q) = query {
        url.push('?');
        url.push_str(q);
    }

    url
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn test_route(host: &str, upstream: &str) -> Route {
        Route {
            id: Uuid::now_v7(),
            name: "test".to_string(),
            host: host.to_string(),
            upstream_url: upstream.to_string(),
            requires_auth: false,
            enabled: true,
            allowed_users: Vec::new(),
        }
    }

    #[test]
    fn test_find_route_by_host() {
        let routes = vec![
            test_route("api.example.com", "http://localhost:3000"),
            test_route("web.example.com", "http://localhost:8080"),
        ];

        let route = find_route(&routes, "api.example.com");
        assert!(route.is_some());
        assert_eq!(route.unwrap().upstream_url, "http://localhost:3000");
    }

    #[test]
    fn test_find_route_by_host_with_port() {
        let routes = vec![
            test_route("api.example.com", "http://localhost:3000"),
        ];

        let route = find_route(&routes, "api.example.com:443");
        assert!(route.is_some());
        assert_eq!(route.unwrap().upstream_url, "http://localhost:3000");
    }

    #[test]
    fn test_build_upstream_url() {
        let route = test_route("api.example.com", "http://localhost:3000");
        let url = build_upstream_url(&route, "/users", None);
        assert_eq!(url, "http://localhost:3000/users");
    }

    #[test]
    fn test_build_upstream_url_with_query() {
        let route = test_route("api.example.com", "http://localhost:3000");
        let url = build_upstream_url(&route, "/users", Some("page=1&limit=10"));
        assert_eq!(url, "http://localhost:3000/users?page=1&limit=10");
    }

    #[test]
    fn test_host_from_origin() {
        assert_eq!(
            host_from_origin("https://gateway.fabi-sc.de/"),
            Some("gateway.fabi-sc.de".to_string())
        );
        assert_eq!(
            host_from_origin("gateway.fabi-sc.de:8443"),
            Some("gateway.fabi-sc.de".to_string())
        );
        assert_eq!(
            host_from_origin("https://Gateway.Fabi-SC.de/_admin"),
            Some("gateway.fabi-sc.de".to_string())
        );
        assert_eq!(host_from_origin("   "), None);
        assert_eq!(host_from_origin(""), None);
    }

    #[test]
    fn test_request_host_ignores_forwarding_headers() {
        let req = actix_web::test::TestRequest::default()
            .insert_header(("Host", "code.fabi-sc.de"))
            .insert_header(("Forwarded", "host=id-dev.fabi-sc.de"))
            .insert_header(("X-Forwarded-Host", "id-dev.fabi-sc.de"))
            .to_http_request();

        assert_eq!(request_host(&req), "code.fabi-sc.de");
    }

    #[test]
    fn test_request_host_strips_port_and_case() {
        let req = actix_web::test::TestRequest::default()
            .insert_header(("Host", "Code.Fabi-SC.de:443"))
            .to_http_request();

        assert_eq!(request_host(&req), "code.fabi-sc.de");
    }
}
