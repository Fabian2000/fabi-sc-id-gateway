//! Admin API endpoints.

use actix_web::{web, HttpRequest, HttpResponse};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::get_session_id;
use crate::config::{IdConfig, Route};
use crate::db::DbPool;
use crate::proxy::ProxyState;

/// Check if request is authenticated.
async fn check_auth(req: &HttpRequest, pool: &DbPool) -> bool {
    let session_id = get_session_id(req);
    if let Some(id) = session_id {
        crate::auth::get_session(pool, &id)
            .await
            .ok()
            .flatten()
            .is_some()
    } else {
        false
    }
}

/// Check if request host matches admin_origin.
async fn check_admin_host(req: &HttpRequest, state: &web::Data<Arc<ProxyState>>) -> bool {
    let host = crate::proxy::request_host(req);
    state.is_admin_host(&host).await
}

/// Username behind the request's session, for audit records.
async fn session_username(req: &HttpRequest, pool: &DbPool) -> Option<String> {
    let session_id = get_session_id(req)?;
    crate::auth::get_session(pool, &session_id)
        .await
        .ok()
        .flatten()
        .map(|s| s.username)
}

/// List all routes.
pub async fn list_routes(
    req: HttpRequest,
    pool: web::Data<DbPool>,
    state: web::Data<Arc<ProxyState>>,
) -> HttpResponse {
    if !check_admin_host(&req, &state).await {
        return HttpResponse::NotFound().json(serde_json::json!({"error": "Not Found"}));
    }
    if !check_auth(&req, pool.get_ref()).await {
        return HttpResponse::Unauthorized().json(serde_json::json!({"error": "Unauthorized"}));
    }

    match crate::db::get_all_routes(pool.get_ref()).await {
        Ok(routes) => HttpResponse::Ok().json(routes),
        Err(e) => HttpResponse::InternalServerError()
            .json(serde_json::json!({"error": e.to_string()})),
    }
}

#[derive(Deserialize)]
pub struct CreateRouteRequest {
    name: String,
    host: String,
    upstream_url: String,
    requires_auth: Option<bool>,
    allowed_users: Option<Vec<String>>,
}

/// Create a new route.
pub async fn create_route(
    req: HttpRequest,
    pool: web::Data<DbPool>,
    state: web::Data<Arc<ProxyState>>,
    body: web::Json<CreateRouteRequest>,
) -> HttpResponse {
    if !check_admin_host(&req, &state).await {
        return HttpResponse::NotFound().json(serde_json::json!({"error": "Not Found"}));
    }
    if !check_auth(&req, pool.get_ref()).await {
        return HttpResponse::Unauthorized().json(serde_json::json!({"error": "Unauthorized"}));
    }

    let route = Route {
        id: Uuid::now_v7(),
        name: body.name.clone(),
        host: body.host.clone(),
        upstream_url: body.upstream_url.clone(),
        // Default deny: a route that omits requires_auth must not be published
        // to the internet unauthenticated.
        requires_auth: body.requires_auth.unwrap_or(true),
        enabled: true,
        allowed_users: body.allowed_users.clone().unwrap_or_default(),
    };

    match crate::db::create_route(pool.get_ref(), &route).await {
        Ok(_) => {
            crate::db::audit(
                pool.get_ref(),
                "route.create",
                &format!(
                    "host={} upstream={} requires_auth={}",
                    route.host, route.upstream_url, route.requires_auth
                ),
                session_username(&req, pool.get_ref()).await.as_deref(),
                Some(&crate::proxy::client_ip(&req)),
            )
            .await;
            HttpResponse::Created().json(route)
        }
        Err(e) => HttpResponse::InternalServerError()
            .json(serde_json::json!({"error": e.to_string()})),
    }
}

#[derive(Deserialize)]
pub struct UpdateRouteRequest {
    name: Option<String>,
    host: Option<String>,
    upstream_url: Option<String>,
    requires_auth: Option<bool>,
    enabled: Option<bool>,
    allowed_users: Option<Vec<String>>,
}

/// Update an existing route.
pub async fn update_route(
    req: HttpRequest,
    pool: web::Data<DbPool>,
    state: web::Data<Arc<ProxyState>>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateRouteRequest>,
) -> HttpResponse {
    if !check_admin_host(&req, &state).await {
        return HttpResponse::NotFound().json(serde_json::json!({"error": "Not Found"}));
    }
    if !check_auth(&req, pool.get_ref()).await {
        return HttpResponse::Unauthorized().json(serde_json::json!({"error": "Unauthorized"}));
    }

    let route_id = path.into_inner();

    // Get existing route
    let routes = match crate::db::get_all_routes(pool.get_ref()).await {
        Ok(r) => r,
        Err(e) => {
            return HttpResponse::InternalServerError()
                .json(serde_json::json!({"error": e.to_string()}))
        }
    };

    let existing = routes.iter().find(|r| r.id == route_id);
    let existing = match existing {
        Some(r) => r.clone(),
        None => return HttpResponse::NotFound().json(serde_json::json!({"error": "Route not found"})),
    };

    let updated = Route {
        id: route_id,
        name: body.name.clone().unwrap_or(existing.name),
        host: body.host.clone().unwrap_or(existing.host),
        upstream_url: body.upstream_url.clone().unwrap_or(existing.upstream_url),
        requires_auth: body.requires_auth.unwrap_or(existing.requires_auth),
        enabled: body.enabled.unwrap_or(existing.enabled),
        allowed_users: body.allowed_users.clone().unwrap_or(existing.allowed_users),
    };

    match crate::db::update_route(pool.get_ref(), &updated).await {
        Ok(_) => {
            crate::db::audit(
                pool.get_ref(),
                "route.update",
                &format!(
                    "host={} upstream={} requires_auth={} enabled={}",
                    updated.host, updated.upstream_url, updated.requires_auth, updated.enabled
                ),
                session_username(&req, pool.get_ref()).await.as_deref(),
                Some(&crate::proxy::client_ip(&req)),
            )
            .await;
            HttpResponse::Ok().json(updated)
        }
        Err(e) => HttpResponse::InternalServerError()
            .json(serde_json::json!({"error": e.to_string()})),
    }
}

/// Delete a route.
pub async fn delete_route(
    req: HttpRequest,
    pool: web::Data<DbPool>,
    state: web::Data<Arc<ProxyState>>,
    path: web::Path<Uuid>,
) -> HttpResponse {
    if !check_admin_host(&req, &state).await {
        return HttpResponse::NotFound().json(serde_json::json!({"error": "Not Found"}));
    }
    if !check_auth(&req, pool.get_ref()).await {
        return HttpResponse::Unauthorized().json(serde_json::json!({"error": "Unauthorized"}));
    }

    let route_id = path.into_inner();

    match crate::db::delete_route(pool.get_ref(), route_id).await {
        Ok(_) => {
            crate::db::audit(
                pool.get_ref(),
                "route.delete",
                &format!("id={}", route_id),
                session_username(&req, pool.get_ref()).await.as_deref(),
                Some(&crate::proxy::client_ip(&req)),
            )
            .await;
            HttpResponse::NoContent().finish()
        }
        Err(e) => HttpResponse::InternalServerError()
            .json(serde_json::json!({"error": e.to_string()})),
    }
}

/// Get ID configuration.
pub async fn get_id_config(
    req: HttpRequest,
    pool: web::Data<DbPool>,
    state: web::Data<Arc<ProxyState>>,
) -> HttpResponse {
    if !check_admin_host(&req, &state).await {
        return HttpResponse::NotFound().json(serde_json::json!({"error": "Not Found"}));
    }
    if !check_auth(&req, pool.get_ref()).await {
        return HttpResponse::Unauthorized().json(serde_json::json!({"error": "Unauthorized"}));
    }

    match crate::db::get_id_config(pool.get_ref()).await {
        Ok(config) => {
            // Don't expose the full API key
            HttpResponse::Ok().json(serde_json::json!({
                "server_url": config.server_url,
                "app_id": config.app_id,
                "api_key_set": !config.api_key.is_empty(),
                "admin_origin": config.admin_origin,
            }))
        }
        Err(e) => HttpResponse::InternalServerError()
            .json(serde_json::json!({"error": e.to_string()})),
    }
}

#[derive(Deserialize)]
pub struct UpdateIdConfigRequest {
    server_url: Option<String>,
    app_id: Option<String>,
    api_key: Option<String>,
    admin_origin: Option<String>,
}

/// Update ID configuration.
pub async fn update_id_config(
    req: HttpRequest,
    pool: web::Data<DbPool>,
    state: web::Data<Arc<ProxyState>>,
    body: web::Json<UpdateIdConfigRequest>,
) -> HttpResponse {
    if !check_admin_host(&req, &state).await {
        return HttpResponse::NotFound().json(serde_json::json!({"error": "Not Found"}));
    }
    if !check_auth(&req, pool.get_ref()).await {
        return HttpResponse::Unauthorized().json(serde_json::json!({"error": "Unauthorized"}));
    }

    let current = match crate::db::get_id_config(pool.get_ref()).await {
        Ok(c) => c,
        Err(e) => {
            return HttpResponse::InternalServerError()
                .json(serde_json::json!({"error": e.to_string()}))
        }
    };

    let updated = IdConfig {
        server_url: body.server_url.clone().unwrap_or(current.server_url),
        app_id: body.app_id.clone().unwrap_or(current.app_id),
        api_key: body.api_key.clone().unwrap_or(current.api_key),
        admin_origin: body.admin_origin.clone().unwrap_or(current.admin_origin),
    };

    match crate::db::set_id_config(pool.get_ref(), &updated).await {
        Ok(_) => {
            crate::db::audit(
                pool.get_ref(),
                "config.update",
                &format!(
                    "server_url={} app_id={} admin_origin={}",
                    updated.server_url, updated.app_id, updated.admin_origin
                ),
                session_username(&req, pool.get_ref()).await.as_deref(),
                Some(&crate::proxy::client_ip(&req)),
            )
            .await;
            HttpResponse::Ok().json(serde_json::json!({"success": true}))
        }
        Err(e) => HttpResponse::InternalServerError()
            .json(serde_json::json!({"error": e.to_string()})),
    }
}

/// Reload routes from database.
pub async fn reload_routes(
    req: HttpRequest,
    pool: web::Data<DbPool>,
    state: web::Data<Arc<ProxyState>>,
) -> HttpResponse {
    if !check_admin_host(&req, &state).await {
        return HttpResponse::NotFound().json(serde_json::json!({"error": "Not Found"}));
    }
    if !check_auth(&req, pool.get_ref()).await {
        return HttpResponse::Unauthorized().json(serde_json::json!({"error": "Unauthorized"}));
    }

    match state.reload_routes().await {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({"success": true})),
        Err(e) => HttpResponse::InternalServerError()
            .json(serde_json::json!({"error": e.to_string()})),
    }
}
