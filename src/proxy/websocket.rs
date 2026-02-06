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

    // Build WebSocket request with all relevant browser headers
    let mut ws_request_builder = tokio_tungstenite::tungstenite::http::Request::builder()
        .uri(&upstream_url)
        .header("Host", upstream_url.split("://").nth(1).and_then(|s| s.split('/').next()).unwrap_or(""))
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .header("Sec-WebSocket-Version", "13")
        .header("Sec-WebSocket-Key", tokio_tungstenite::tungstenite::handshake::client::generate_key());

    // Forward ALL headers from browser to upstream, except hop-by-hop headers
    for (name, value) in req.headers().iter() {
        let name_str = name.as_str().to_lowercase();

        // Skip hop-by-hop headers that are already set above
        if name_str == "host" || name_str == "connection" || name_str == "upgrade"
            || name_str == "sec-websocket-version" || name_str == "sec-websocket-key"
            || name_str == "sec-websocket-extensions" || name_str == "sec-websocket-protocol" {
            continue;
        }

        if let Ok(value_str) = value.to_str() {
            ws_request_builder = ws_request_builder.header(name.as_str(), value_str);
        }
    }

    let ws_request = ws_request_builder.body(())
        .map_err(|e| GatewayError::Proxy(format!("Failed to build WebSocket request: {}", e)))?;

    // Connect to upstream WebSocket
    let (upstream_ws, upstream_response) = connect_async(ws_request).await.map_err(|e| {
        error!("Failed to connect to upstream WebSocket: {}", e);
        GatewayError::Upstream(format!("WebSocket connection failed: {}", e))
    })?;

    let (mut upstream_sink, mut upstream_stream) = upstream_ws.split();

    // Accept client WebSocket connection
    let (mut response, client_session, client_stream) =
        actix_ws::handle(&req, stream).map_err(|e| {
            error!("Failed to accept WebSocket: {}", e);
            GatewayError::Proxy(format!("WebSocket upgrade failed: {}", e))
        })?;

    // Increase frame size limit to 100MB for large messages
    let mut client_stream = client_stream.max_frame_size(100 * 1024 * 1024);

    // Forward Set-Cookie headers from upstream to browser
    for cookie in upstream_response.headers().get_all("set-cookie") {
        if let Ok(cookie_str) = cookie.to_str() {
            response.headers_mut().insert(
                actix_web::http::header::SET_COOKIE,
                actix_web::http::header::HeaderValue::from_str(cookie_str)
                    .unwrap_or_else(|_| actix_web::http::header::HeaderValue::from_static("")),
            );
        }
    }

    // Spawn coordinated bi-directional forwarding task
    actix_rt::spawn(async move {
        // Wrap session in Option so we can consume it when closing
        let mut client_session = Some(client_session);

        // State for assembling fragmented messages from client
        let mut client_fragments: Vec<actix_web::web::Bytes> = Vec::new();
        let mut client_is_text: bool = false;

        loop {
            tokio::select! {
                // Forward client -> upstream
                client_msg = client_stream.next() => {
                    match client_msg {
                        Some(Ok(msg)) => {
                            // Handle message with fragment assembly
                            let should_continue = match msg {
                                WsMessage::Text(text) => {
                                    if let Err(e) = upstream_sink.send(TungMessage::Text(text.to_string().into())).await {
                                        error!("Failed to send TEXT to upstream: {}", e);
                                        false
                                    } else {
                                        true
                                    }
                                }
                                WsMessage::Binary(bin) => {
                                    if let Err(e) = upstream_sink.send(TungMessage::Binary(bin)).await {
                                        error!("Failed to send BINARY to upstream: {}", e);
                                        false
                                    } else {
                                        true
                                    }
                                }
                                WsMessage::Ping(data) => {
                                    if let Err(e) = upstream_sink.send(TungMessage::Ping(data)).await {
                                        error!("Failed to send PING to upstream: {}", e);
                                        false
                                    } else {
                                        true
                                    }
                                }
                                WsMessage::Pong(data) => {
                                    if let Err(e) = upstream_sink.send(TungMessage::Pong(data)).await {
                                        error!("Failed to send PONG to upstream: {}", e);
                                        false
                                    } else {
                                        true
                                    }
                                }
                                WsMessage::Close(reason) => {
                                    let _ = upstream_sink.send(TungMessage::Close(reason.map(|r| {
                                        tokio_tungstenite::tungstenite::protocol::CloseFrame {
                                            code: tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode::from(u16::from(r.code)),
                                            reason: r.description.map(|d| d.to_string().into()).unwrap_or_default(),
                                        }
                                    }))).await;
                                    false
                                }
                                WsMessage::Continuation(item) => {
                                    // Handle fragmented messages
                                    match item {
                                        actix_ws::Item::FirstText(data) => {
                                            client_fragments.clear();
                                            client_fragments.push(data);
                                            client_is_text = true;
                                            true
                                        }
                                        actix_ws::Item::FirstBinary(data) => {
                                            client_fragments.clear();
                                            client_fragments.push(data);
                                            client_is_text = false;
                                            true
                                        }
                                        actix_ws::Item::Continue(data) => {
                                            client_fragments.push(data);
                                            true
                                        }
                                        actix_ws::Item::Last(data) => {
                                            client_fragments.push(data);

                                            // Assemble complete message
                                            let total_len: usize = client_fragments.iter().map(|b| b.len()).sum();
                                            let mut complete = Vec::with_capacity(total_len);
                                            for fragment in &client_fragments {
                                                complete.extend_from_slice(fragment);
                                            }
                                            let complete_bytes = actix_web::web::Bytes::from(complete);

                                            // Send as appropriate type
                                            let result = if client_is_text {
                                                match String::from_utf8(complete_bytes.to_vec()) {
                                                    Ok(text) => {
                                                        upstream_sink.send(TungMessage::Text(text.into())).await
                                                    }
                                                    Err(e) => {
                                                        error!("Failed to decode fragmented text message: {}", e);
                                                        upstream_sink.send(TungMessage::Binary(complete_bytes)).await
                                                    }
                                                }
                                            } else {
                                                upstream_sink.send(TungMessage::Binary(complete_bytes)).await
                                            };

                                            client_fragments.clear();

                                            if let Err(e) = result {
                                                error!("Failed to send assembled message to upstream: {}", e);
                                                false
                                            } else {
                                                true
                                            }
                                        }
                                    }
                                }
                                WsMessage::Nop => true,
                            };

                            if !should_continue {
                                break;
                            }
                        }
                        Some(Err(e)) => {
                            warn!("Client WebSocket error: {}", e);
                            break;
                        }
                        None => {
                            debug!("Client WebSocket stream closed");
                            break;
                        }
                    }
                }
                // Forward upstream -> client
                upstream_msg = upstream_stream.next() => {
                    match upstream_msg {
                        Some(Ok(msg)) => {
                            // Handle close frames specially to forward them properly
                            if matches!(msg, TungMessage::Close(_)) {
                                if let TungMessage::Close(frame) = msg {
                                    let close_reason = frame.map(|f| actix_ws::CloseReason {
                                        code: actix_ws::CloseCode::from(u16::from(f.code)),
                                        description: if f.reason.is_empty() {
                                            None
                                        } else {
                                            Some(f.reason.to_string().into())
                                        },
                                    });
                                    if let Some(session) = client_session.take() {
                                        let _ = session.close(close_reason).await;
                                    }
                                }
                                break;
                            }

                            if let Some(ref mut session) = client_session {
                                if !handle_upstream_message(msg, session).await {
                                    break;
                                }
                            } else {
                                break;
                            }
                        }
                        Some(Err(e)) => {
                            warn!("Upstream WebSocket error: {}", e);
                            break;
                        }
                        None => {
                            debug!("Upstream WebSocket stream closed");
                            break;
                        }
                    }
                }
            }
        }

        // Clean up: close both connections gracefully
        // Try to flush any pending messages before closing
        if let Err(e) = upstream_sink.close().await {
            debug!("Upstream sink close error (may already be closed): {}", e);
        }
        // Client session might already be closed if we sent a close frame
        if let Some(session) = client_session {
            if let Err(e) = session.close(None).await {
                debug!("Client session close error (may already be closed): {}", e);
            }
        }
        debug!("WebSocket proxy task terminated");
    });

    Ok(response)
}


/// Handle a message from upstream and forward to client
async fn handle_upstream_message(
    msg: TungMessage,
    client_session: &mut actix_ws::Session,
) -> bool {
    match msg {
        TungMessage::Text(text) => {
            if let Err(e) = client_session.text(text.to_string()).await {
                error!("Failed to send TEXT to client: {:?}", e);
                return false;
            }
        }
        TungMessage::Binary(bin) => {
            if let Err(e) = client_session.binary(bin.to_vec()).await {
                error!("Failed to send BINARY to client: {:?}", e);
                return false;
            }
        }
        TungMessage::Ping(data) => {
            if let Err(e) = client_session.ping(&data).await {
                error!("Failed to send PING to client: {:?}", e);
                return false;
            }
        }
        TungMessage::Pong(data) => {
            if let Err(e) = client_session.pong(&data).await {
                error!("Failed to send PONG to client: {:?}", e);
                return false;
            }
        }
        TungMessage::Close(_frame) => {
            // Signal close - main loop will handle forwarding to client
            return false;
        }
        TungMessage::Frame(_) => {
            // Raw frames are not expected in normal operation
        }
    }
    true
}
