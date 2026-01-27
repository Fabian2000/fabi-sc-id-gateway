//! WebSocket proxy support.
//!
//! This module handles WebSocket upgrade requests and proxies them to upstream servers.

use actix_web::{web, HttpRequest, HttpResponse};
use actix_ws::Message as WsMessage;
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::{connect_async, tungstenite::Message as TungMessage};
use tracing::{debug, error, warn};

use crate::config::Route;
use crate::error::GatewayError;

/// Check if a request is a WebSocket upgrade request.
pub fn is_websocket_upgrade(req: &HttpRequest) -> bool {
    req.headers()
        .get("upgrade")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.eq_ignore_ascii_case("websocket"))
        .unwrap_or(false)
}

/// Handle WebSocket upgrade and proxy to upstream.
pub async fn websocket_proxy(
    req: HttpRequest,
    stream: web::Payload,
    route: &Route,
    path: &str,
    query: &str,
) -> Result<HttpResponse, GatewayError> {
    // Build upstream WebSocket URL
    let upstream_base = route.upstream_url.trim_end_matches('/');
    let upstream_url = if upstream_base.starts_with("https://") {
        format!("wss://{}{}", &upstream_base[8..], path)
    } else if upstream_base.starts_with("http://") {
        format!("ws://{}{}", &upstream_base[7..], path)
    } else {
        format!("ws://{}{}", upstream_base, path)
    };

    let upstream_url = if query.is_empty() {
        upstream_url
    } else {
        format!("{}?{}", upstream_url, query)
    };

    debug!("WebSocket proxy: upgrading connection to {}", upstream_url);

    // Connect to upstream WebSocket
    let (upstream_ws, _) = connect_async(&upstream_url).await.map_err(|e| {
        error!("Failed to connect to upstream WebSocket: {}", e);
        GatewayError::Upstream(format!("WebSocket connection failed: {}", e))
    })?;

    let (mut upstream_sink, mut upstream_stream) = upstream_ws.split();

    // Accept client WebSocket connection
    let (response, mut client_session, mut client_stream) =
        actix_ws::handle(&req, stream).map_err(|e| {
            error!("Failed to accept WebSocket: {}", e);
            GatewayError::Proxy(format!("WebSocket upgrade failed: {}", e))
        })?;

    // Spawn task to forward messages from client to upstream
    actix_rt::spawn(async move {
        // Forward client -> upstream
        while let Some(msg) = client_stream.next().await {
            match msg {
                Ok(WsMessage::Text(text)) => {
                    if upstream_sink
                        .send(TungMessage::Text(text.to_string().into()))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Ok(WsMessage::Binary(bin)) => {
                    if upstream_sink
                        .send(TungMessage::Binary(bin.to_vec().into()))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Ok(WsMessage::Ping(data)) => {
                    if upstream_sink
                        .send(TungMessage::Ping(data.to_vec().into()))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Ok(WsMessage::Pong(data)) => {
                    if upstream_sink
                        .send(TungMessage::Pong(data.to_vec().into()))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Ok(WsMessage::Close(reason)) => {
                    let _ = upstream_sink
                        .send(TungMessage::Close(reason.map(|r| {
                            tokio_tungstenite::tungstenite::protocol::CloseFrame {
                                code: tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode::from(u16::from(r.code)),
                                reason: r.description.map(|d| d.to_string().into()).unwrap_or_default(),
                            }
                        })))
                        .await;
                    break;
                }
                Ok(WsMessage::Continuation(_)) | Ok(WsMessage::Nop) => {}
                Err(e) => {
                    warn!("Client WebSocket error: {}", e);
                    break;
                }
            }
        }
        let _ = upstream_sink.close().await;
    });

    // Forward upstream -> client
    actix_rt::spawn(async move {
        while let Some(msg) = upstream_stream.next().await {
            match msg {
                Ok(TungMessage::Text(text)) => {
                    if client_session.text(text.to_string()).await.is_err() {
                        break;
                    }
                }
                Ok(TungMessage::Binary(bin)) => {
                    if client_session.binary(bin.to_vec()).await.is_err() {
                        break;
                    }
                }
                Ok(TungMessage::Ping(data)) => {
                    if client_session.ping(&data).await.is_err() {
                        break;
                    }
                }
                Ok(TungMessage::Pong(data)) => {
                    if client_session.pong(&data).await.is_err() {
                        break;
                    }
                }
                Ok(TungMessage::Close(frame)) => {
                    let _ = client_session
                        .close(frame.map(|f| actix_ws::CloseReason {
                            code: actix_ws::CloseCode::from(u16::from(f.code)),
                            description: Some(f.reason.to_string()),
                        }))
                        .await;
                    break;
                }
                Ok(TungMessage::Frame(_)) => {}
                Err(e) => {
                    warn!("Upstream WebSocket error: {}", e);
                    let _ = client_session
                        .close(Some(actix_ws::CloseReason {
                            code: actix_ws::CloseCode::Error,
                            description: Some("Upstream error".to_string()),
                        }))
                        .await;
                    break;
                }
            }
        }
    });

    Ok(response)
}
