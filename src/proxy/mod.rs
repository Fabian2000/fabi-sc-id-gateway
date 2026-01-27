//! Reverse proxy module for HTTP, WebSocket, and HTTP/3 support.

mod handler;
mod websocket;

pub use handler::*;
#[allow(unused_imports)]
pub use websocket::*;

use crate::config::Route;

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
}
