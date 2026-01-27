use actix_web::{web, HttpRequest, HttpResponse};
use std::sync::Arc;

use crate::auth::{clear_session_cookie, create_session_cookie, get_session_id, IdClient, Session};
use crate::db::DbPool;
use crate::proxy::ProxyState;

use super::templates;

fn error_response(status: u16, message: &str) -> HttpResponse {
    HttpResponse::build(actix_web::http::StatusCode::from_u16(status).unwrap())
        .content_type("text/html; charset=utf-8")
        .body(templates::error_page(status, message))
}

fn redirect(location: &str) -> HttpResponse {
    HttpResponse::Found()
        .insert_header(("Location", location))
        .finish()
}

/// Returns session if valid. Periodically validates token at ID service (every 5 min).
/// Returns None if session doesn't exist or token was revoked.
async fn get_session_from_request(req: &HttpRequest, pool: &DbPool) -> Option<Session> {
    let session_id = get_session_id(req)?;
    let session = crate::auth::get_session(pool, &session_id).await.ok().flatten()?;

    if session.needs_validation() {
        let config = crate::db::get_id_config(pool).await.ok()?;
        let client = IdClient::new(config);

        // Validate token at ID service
        if client.get_user(&session.token, &client.admin_origin()).await.is_err() {
            // Token invalid/revoked - delete session
            let _ = crate::auth::delete_session(pool, &session_id).await;
            return None;
        }

        // Update last_validated timestamp
        let _ = crate::auth::update_last_validated(pool, &session_id).await;
    }

    Some(session)
}

async fn check_admin_host(req: &HttpRequest, state: &web::Data<Arc<ProxyState>>) -> Option<HttpResponse> {
    let host = req.connection_info().host().to_string();
    if !state.is_admin_host(&host).await {
        Some(error_response(404, "Not Found"))
    } else {
        None
    }
}

pub async fn admin_dashboard(
    req: HttpRequest,
    pool: web::Data<DbPool>,
    state: web::Data<Arc<ProxyState>>,
) -> HttpResponse {
    if let Some(response) = check_admin_host(&req, &state).await {
        return response;
    }

    if !crate::db::is_id_configured(pool.get_ref()).await.unwrap_or(false) {
        return redirect("/_admin/setup");
    }

    let session = match get_session_from_request(&req, pool.get_ref()).await {
        Some(s) => s,
        None => return redirect("/_admin/login"),
    };

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(templates::dashboard(&session.username))
}

pub async fn login_page(
    req: HttpRequest,
    pool: web::Data<DbPool>,
    state: web::Data<Arc<ProxyState>>,
) -> HttpResponse {
    if let Some(response) = check_admin_host(&req, &state).await {
        return response;
    }

    if !crate::db::is_id_configured(pool.get_ref()).await.unwrap_or(false) {
        return redirect("/_admin/setup");
    }

    let config = match crate::db::get_id_config(pool.get_ref()).await {
        Ok(c) => c,
        Err(_) => return error_response(500, "Internal Server Error"),
    };

    let client = IdClient::new(config);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(templates::login(&client.consent_url("/_admin/callback")))
}

/// OAuth callback handler.
/// Handles both admin login and protected route login via popup.
/// If state parameter is present, it contains the origin of the protected route.
pub async fn auth_callback(
    req: HttpRequest,
    pool: web::Data<DbPool>,
    state: web::Data<Arc<ProxyState>>,
) -> HttpResponse {
    // Check admin host
    if let Some(response) = check_admin_host(&req, &state).await {
        return response;
    }
    let query = web::Query::<std::collections::HashMap<String, String>>::from_query(req.query_string());
    let code = match query {
        Ok(q) => q.get("code").cloned(),
        Err(_) => None,
    };

    let code = match code {
        Some(c) => c,
        None => return error_response(400, "Bad Request"),
    };

    let config = match crate::db::get_id_config(pool.get_ref()).await {
        Ok(c) => c,
        Err(_) => return error_response(500, "Internal Server Error"),
    };

    let client = IdClient::new(config);

    // Get origin from request (admin origin)
    let conn_info = req.connection_info();
    let origin = format!("{}://{}", conn_info.scheme(), conn_info.host());
    tracing::info!("Exchange with origin: {}", origin);

    let exchange = match client.exchange_code(&code, &origin).await {
        Ok(e) => e,
        Err(e) => {
            tracing::error!("Exchange failed: {}", e);
            return error_response(400, "Bad Request");
        }
    };

    let user = match client.get_user(&exchange.token, &origin).await {
        Ok(u) => u,
        Err(e) => {
            tracing::error!("Get user failed: {}", e);
            return error_response(400, "Bad Request");
        }
    };

    let username = match &user.username {
        Some(u) => u,
        None => return error_response(400, "Missing username scope"),
    };

    // postMessage target origin is '*' because we can't pass state through ID service.
    // Security: the receiver validates event.origin before accepting the message.
    let target_origin = "*";

    // Return HTML that detects popup vs normal window
    // Admin check happens in callback-session for normal login flow
    // If popup (window.opener exists): send postMessage with token and close
    // If normal window: create admin session and redirect
    let html = format!(
        r#"<!DOCTYPE html>
<html>
<head><title>Login</title></head>
<body>
<script>
if (window.opener) {{
    // Popup mode: send token to opener and close
    window.opener.postMessage({{
        type: 'login_success',
        user_id: '{user_id}',
        username: '{username}',
        token: '{token}'
    }}, '{target_origin}');
    window.close();
}} else {{
    // Normal mode: redirect to create session
    window.location.href = '/_admin/callback-session?token={token}';
}}
</script>
<noscript>JavaScript is required.</noscript>
</body>
</html>"#,
        user_id = user.id,
        username = username,
        token = exchange.token,
        target_origin = target_origin
    );

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html)
}

/// Creates admin session from token (for normal login flow, not popup).
pub async fn auth_callback_session(
    req: HttpRequest,
    pool: web::Data<DbPool>,
    state: web::Data<Arc<ProxyState>>,
) -> HttpResponse {
    if let Some(response) = check_admin_host(&req, &state).await {
        return response;
    }

    let query =
        web::Query::<std::collections::HashMap<String, String>>::from_query(req.query_string());
    let token = match query {
        Ok(q) => q.get("token").cloned(),
        Err(_) => None,
    };

    let token = match token {
        Some(t) => t,
        None => return error_response(400, "Bad Request"),
    };

    let config = match crate::db::get_id_config(pool.get_ref()).await {
        Ok(c) => c,
        Err(_) => return error_response(500, "Internal Server Error"),
    };

    let client = IdClient::new(config);
    let conn_info = req.connection_info();
    let origin = format!("{}://{}", conn_info.scheme(), conn_info.host());

    let user = match client.get_user(&token, &origin).await {
        Ok(u) => u,
        Err(_) => return error_response(400, "Invalid token"),
    };

    let username = match &user.username {
        Some(u) => u,
        None => return error_response(400, "Missing username"),
    };

    let is_admin = crate::db::is_admin(pool.get_ref(), username)
        .await
        .unwrap_or(false);

    if !is_admin {
        return error_response(403, "Forbidden");
    }

    let session = match crate::auth::create_session(pool.get_ref(), &user.id, username, &token)
        .await
    {
        Ok(s) => s,
        Err(_) => return error_response(500, "Internal Server Error"),
    };

    let is_secure = conn_info.scheme() == "https";
    HttpResponse::Found()
        .cookie(create_session_cookie(&session.id, is_secure))
        .insert_header(("Location", "/_admin"))
        .finish()
}

pub async fn logout(
    req: HttpRequest,
    pool: web::Data<DbPool>,
    state: web::Data<Arc<ProxyState>>,
) -> HttpResponse {
    if let Some(response) = check_admin_host(&req, &state).await {
        return response;
    }

    if let Some(session_id) = get_session_id(&req) {
        if let Ok(Some(session)) = crate::auth::get_session(pool.get_ref(), &session_id).await {
            if let Ok(config) = crate::db::get_id_config(pool.get_ref()).await {
                let client = IdClient::new(config);
                match client.revoke_token(&session.token, &client.admin_origin()).await {
                    Ok(_) => tracing::debug!("Token revoked for user: {}", session.username),
                    Err(e) => tracing::warn!("Failed to revoke token for user {}: {}", session.username, e),
                }
            }
        }
        let _ = crate::auth::delete_session(pool.get_ref(), &session_id).await;
    }

    HttpResponse::Found()
        .cookie(clear_session_cookie())
        .insert_header(("Location", "/_admin/login"))
        .finish()
}

/// First-run setup page. Accessible without auth if not yet configured.
pub async fn setup_page(
    req: HttpRequest,
    pool: web::Data<DbPool>,
    state: web::Data<Arc<ProxyState>>,
) -> HttpResponse {
    let is_configured = crate::db::is_id_configured(pool.get_ref())
        .await
        .unwrap_or(false);

    if is_configured {
        if let Some(response) = check_admin_host(&req, &state).await {
            return response;
        }
        return redirect("/_admin");
    }

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(templates::setup())
}

pub async fn setup_submit(
    pool: web::Data<DbPool>,
    _state: web::Data<Arc<ProxyState>>,
    form: web::Form<SetupForm>,
) -> HttpResponse {
    let config = crate::config::IdConfig {
        server_url: "https://id.fabi-sc.de".to_string(),
        app_id: form.app_id.clone(),
        api_key: form.api_key.clone(),
        admin_origin: form.admin_origin.clone(),
    };

    if crate::db::set_id_config(pool.get_ref(), &config).await.is_err() {
        return error_response(500, "Internal Server Error");
    }

    let admin_users: Vec<&str> = form
        .admin_users
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    if admin_users.is_empty() {
        return error_response(400, "Bad Request");
    }

    if crate::db::set_admins(pool.get_ref(), &admin_users).await.is_err() {
        return error_response(500, "Internal Server Error");
    }

    redirect("/_admin/login")
}

#[derive(serde::Deserialize)]
pub struct SetupForm {
    app_id: String,
    api_key: String,
    admin_origin: String,
    admin_users: String,
}

pub async fn routes_page(
    req: HttpRequest,
    pool: web::Data<DbPool>,
    state: web::Data<Arc<ProxyState>>,
) -> HttpResponse {
    if let Some(response) = check_admin_host(&req, &state).await {
        return response;
    }

    if get_session_from_request(&req, pool.get_ref()).await.is_none() {
        return redirect("/_admin/login");
    }

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(templates::routes())
}

pub async fn settings_page(
    req: HttpRequest,
    pool: web::Data<DbPool>,
    state: web::Data<Arc<ProxyState>>,
) -> HttpResponse {
    if let Some(response) = check_admin_host(&req, &state).await {
        return response;
    }

    if get_session_from_request(&req, pool.get_ref()).await.is_none() {
        return redirect("/_admin/login");
    }

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(templates::settings())
}
