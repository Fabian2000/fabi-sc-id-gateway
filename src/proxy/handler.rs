//! HTTP proxy handler.

use actix_web::{cookie::{Cookie, SameSite, time::Duration as CookieDuration}, web, HttpRequest, HttpResponse};
use futures_util::StreamExt;
use reqwest::Client;
use std::sync::Arc;
use tracing::{debug, error};

use crate::config::Route;
use crate::db::DbPool;
use crate::error::GatewayError;

/// Shared state for the proxy handler.
pub struct ProxyState {
    pub client: Client,
    pub pool: DbPool,
    pub routes: tokio::sync::RwLock<Vec<Route>>,
    pub admin_host: tokio::sync::RwLock<Option<String>>,
}

impl ProxyState {
    pub fn new(pool: DbPool) -> Self {
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            pool,
            routes: tokio::sync::RwLock::new(Vec::new()),
            admin_host: tokio::sync::RwLock::new(None),
        }
    }

    /// Reload routes from the database.
    pub async fn reload_routes(&self) -> anyhow::Result<()> {
        let routes = crate::db::get_enabled_routes(&self.pool).await?;
        let mut guard = self.routes.write().await;
        *guard = routes;

        // Also reload admin_host from settings
        if let Ok(Some(host)) = crate::db::get_setting(&self.pool, "admin_host").await {
            if !host.is_empty() {
                let mut admin_guard = self.admin_host.write().await;
                *admin_guard = Some(host);
            }
        }

        Ok(())
    }

    /// Check if the given host is the admin host.
    pub async fn is_admin_host(&self, host: &str) -> bool {
        let guard = self.admin_host.read().await;
        match &*guard {
            Some(admin_host) => {
                // Strip port from host if present
                let host_without_port = host.split(':').next().unwrap_or(host);
                host_without_port == admin_host
            }
            None => true, // If not configured, allow access (for initial setup)
        }
    }
}

/// Main proxy handler for all incoming requests.
pub async fn proxy_handler(
    req: HttpRequest,
    mut payload: web::Payload,
    state: web::Data<Arc<ProxyState>>,
) -> Result<HttpResponse, GatewayError> {
    let path = req.path().to_string();
    let query = req.query_string().to_string();
    let conn_info = req.connection_info().clone();
    let host = conn_info.host().to_string();

    // Find matching route by Host header
    let routes = state.routes.read().await;
    let route = match super::find_route(&routes, &host) {
        Some(r) => r.clone(),
        None => {
            // Check if gateway is configured - if not, redirect to setup
            let is_configured = crate::db::is_id_configured(&state.pool)
                .await
                .unwrap_or(false);

            if !is_configured {
                return Ok(HttpResponse::Found()
                    .insert_header(("Location", "/_admin/setup"))
                    .finish());
            }

            debug!("No route found for host: {}", host);
            return Err(GatewayError::RouteNotFound(host.to_string()));
        }
    };
    drop(routes); // Release lock early

    // Check authentication if required
    if route.requires_auth {
        // Get user token from cookie
        let user_token = req
            .cookie("gateway_user_token")
            .map(|c| c.value().to_string());

        // Load ID config for consent URL
        let config = match crate::db::get_id_config(&state.pool).await {
            Ok(c) => c,
            Err(_) => {
                return Ok(HttpResponse::InternalServerError()
                    .content_type("text/html; charset=utf-8")
                    .body(crate::web::templates::error_page(500, "Internal Server Error")));
            }
        };

        let client = crate::auth::IdClient::new(config);
        let admin_origin = client.admin_origin();
        let callback_url = format!("{}/_admin/callback", admin_origin);
        let consent_url = client.consent_url(&callback_url);

        match user_token {
            Some(token) => {
                // Validate token and check whitelist (use admin_origin - token was exchanged with this origin)
                let user = match client.get_user(&token, &admin_origin).await {
                    Ok(u) => u,
                    Err(_) => {
                        // Invalid/expired token: clear stale cookie so browser stops resending it.
                        return Ok(login_required_page(&consent_url, &admin_origin, true));
                    }
                };

                // Check if user is in allowed_users whitelist
                let username = user.username.unwrap_or_default();
                if !route.allowed_users.is_empty() && !route.allowed_users.contains(&username) {
                    return Ok(HttpResponse::Forbidden()
                        .content_type("text/html; charset=utf-8")
                        .body(crate::web::templates::error_page(403, "Access Denied")));
                }
            }
            None => {
                // No token, show login page with button (user interaction for popup)
                return Ok(login_required_page(&consent_url, &admin_origin, false));
            }
        }
    }

    // Check for WebSocket upgrade
    if super::is_websocket_upgrade(&req) {
        debug!("WebSocket upgrade request for {}", path);
        return super::websocket_proxy(req, web::Payload::from(payload), &route, &path, &query).await;
    }

    // Read body from payload for regular HTTP requests
    let mut body = web::BytesMut::new();
    while let Some(chunk) = payload.next().await {
        let chunk = chunk.map_err(|e| GatewayError::Proxy(e.to_string()))?;
        body.extend_from_slice(&chunk);
    }
    let body = body.freeze();

    // Build upstream URL
    let upstream_url = super::build_upstream_url(
        &route,
        &path,
        if query.is_empty() { None } else { Some(&query) },
    );

    debug!("Proxying {} -> {}", &path, upstream_url);

    // Forward the request
    let method = match req.method().as_str() {
        "GET" => reqwest::Method::GET,
        "POST" => reqwest::Method::POST,
        "PUT" => reqwest::Method::PUT,
        "DELETE" => reqwest::Method::DELETE,
        "PATCH" => reqwest::Method::PATCH,
        "HEAD" => reqwest::Method::HEAD,
        "OPTIONS" => reqwest::Method::OPTIONS,
        other => {
            return Err(GatewayError::Proxy(format!(
                "Unsupported method: {}",
                other
            )))
        }
    };

    let mut upstream_req = state.client.request(method, &upstream_url);

    // Forward headers (excluding hop-by-hop headers)
    for (name, value) in req.headers() {
        let name_str = name.as_str().to_lowercase();
        if !is_hop_by_hop_header(&name_str) {
            if let Ok(v) = value.to_str() {
                upstream_req = upstream_req.header(name.as_str(), v);
            }
        }
    }

    // Add X-Forwarded headers
    if let Some(peer) = req.peer_addr() {
        upstream_req = upstream_req.header("X-Forwarded-For", peer.ip().to_string());
    }
    upstream_req = upstream_req.header("X-Forwarded-Proto", conn_info.scheme());
    upstream_req = upstream_req.header("X-Forwarded-Host", conn_info.host());

    // Forward body
    if !body.is_empty() {
        upstream_req = upstream_req.body(body.to_vec());
    }

    // Send request
    let upstream_resp = upstream_req.send().await.map_err(|e| {
        error!("Upstream request failed: {}", e);
        GatewayError::Upstream(e.to_string())
    })?;

    // Build response
    let mut response = HttpResponse::build(
        actix_web::http::StatusCode::from_u16(upstream_resp.status().as_u16())
            .unwrap_or(actix_web::http::StatusCode::INTERNAL_SERVER_ERROR),
    );

    // Forward response headers. Use append (not insert) so multi-valued headers
    // like Set-Cookie are all forwarded — insert would overwrite, dropping every
    // cookie but the last (e.g. an upstream setting auth_token + refresh_token).
    for (name, value) in upstream_resp.headers() {
        let name_str = name.as_str().to_lowercase();
        if !is_hop_by_hop_header(&name_str) {
            if let Ok(v) = value.to_str() {
                response.append_header((name.as_str(), v));
            }
        }
    }

    // Forward body
    let body = upstream_resp.bytes().await.map_err(|e| {
        error!("Failed to read upstream response body: {}", e);
        GatewayError::Upstream(e.to_string())
    })?;

    Ok(response.body(body))
}

/// Generate login required page with popup button.
/// consent_url: The full URL to the ID consent page (e.g., https://id.fabi-sc.com/consent/{app_id}?redirect_uri=...)
/// admin_origin: The origin of the admin/gateway (e.g., https://gateway.fabi-sc.com)
fn login_required_page(consent_url: &str, admin_origin: &str, clear_stale_cookie: bool) -> HttpResponse {
    let html = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Login Required</title>
    <style>
        * {{ margin: 0; padding: 0; box-sizing: border-box; }}
        body {{
            font-family: system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            background: #f5f7fa;
            min-height: 100vh;
            display: flex;
            align-items: center;
            justify-content: center;
        }}
        .login-container {{
            text-align: center;
            padding: 2rem;
            background: white;
            border-radius: 8px;
            box-shadow: 0 2px 10px rgba(0,0,0,0.1);
            max-width: 400px;
        }}
        h1 {{
            color: #1a365d;
            margin-bottom: 1rem;
        }}
        p {{
            color: #666;
            margin-bottom: 1.5rem;
        }}
        .btn {{
            display: inline-block;
            padding: 0.75rem 1.5rem;
            background: #3182ce;
            color: white;
            border: none;
            border-radius: 4px;
            font-size: 1rem;
            cursor: pointer;
            text-decoration: none;
        }}
        .btn:hover {{
            background: #2c5282;
        }}
    </style>
</head>
<body>
    <div class="login-container">
        <h1>Login Required</h1>
        <p>You need to sign in to access this resource.</p>
        <button class="btn" onclick="openLoginPopup()">Sign in with Fabi-SC ID</button>
    </div>
    <script>
        function openLoginPopup() {{
            const popup = window.open('{consent_url}', 'login', 'width=500,height=600');
            window.addEventListener('message', function handler(event) {{
                if (event.origin !== '{admin_origin}') return;
                if (event.data && event.data.type === 'login_success') {{
                    document.cookie = 'gateway_user_token=' + event.data.token + '; path=/; SameSite=Lax';
                    window.removeEventListener('message', handler);
                    window.location.reload();
                }} else if (event.data && event.data.type === 'login_error') {{
                    alert('Login failed: ' + event.data.error);
                    window.removeEventListener('message', handler);
                }}
            }});
        }}
    </script>
</body>
</html>"#,
        consent_url = consent_url,
        admin_origin = admin_origin
    );
    let mut builder = HttpResponse::Unauthorized();
    builder.content_type("text/html; charset=utf-8");
    if clear_stale_cookie {
        // Match attributes of the cookie set in JS (path=/, SameSite=Lax) so the browser removes it.
        let clear = Cookie::build("gateway_user_token", "")
            .path("/")
            .same_site(SameSite::Lax)
            .max_age(CookieDuration::seconds(0))
            .finish();
        builder.cookie(clear);
    }
    builder.body(html)
}

/// Check if a header is a hop-by-hop header that should not be forwarded.
fn is_hop_by_hop_header(name: &str) -> bool {
    matches!(
        name,
        "connection"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailers"
            | "transfer-encoding"
            | "upgrade"
    )
}
