//! Authentication middleware.

use actix_web::cookie::Cookie;
use actix_web::HttpRequest;

/// Session cookie name.
pub const SESSION_COOKIE: &str = "gateway_session";

/// Extract session ID from request cookies.
pub fn get_session_id(req: &HttpRequest) -> Option<String> {
    req.cookie(SESSION_COOKIE).map(|c| c.value().to_string())
}

/// Create a session cookie.
pub fn create_session_cookie(session_id: &str, secure: bool) -> Cookie<'static> {
    Cookie::build(SESSION_COOKIE, session_id.to_string())
        .path("/")
        .http_only(true)
        .secure(secure)
        .same_site(actix_web::cookie::SameSite::Lax)
        .max_age(actix_web::cookie::time::Duration::hours(24))
        .finish()
}

/// Create a cookie that clears the session.
pub fn clear_session_cookie() -> Cookie<'static> {
    Cookie::build(SESSION_COOKIE, "")
        .path("/")
        .http_only(true)
        .max_age(actix_web::cookie::time::Duration::ZERO)
        .finish()
}
