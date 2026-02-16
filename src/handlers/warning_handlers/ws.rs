use actix_session::Session;
use actix_web::{web, HttpRequest, HttpResponse};
use actix_ws::Message;
use std::collections::HashMap;
use std::sync::RwLock;
use tokio::sync::mpsc;

use crate::auth::session::get_user_id;
use crate::db::DbPool;
use crate::warnings::queries;

pub type ConnectionMap = std::sync::Arc<RwLock<HashMap<i64, Vec<mpsc::UnboundedSender<String>>>>>;

pub fn new_connection_map() -> ConnectionMap {
    std::sync::Arc::new(RwLock::new(HashMap::new()))
}

/// Notify connected users about a new warning.
pub fn notify_users(
    conn_map: &ConnectionMap,
    pool: &DbPool,
    target_user_ids: &[i64],
    warning_id: i64,
    severity: &str,
    title: &str,
) {
    let conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return,
    };
    let map = match conn_map.read() {
        Ok(m) => m,
        Err(_) => return,
    };
    for &user_id in target_user_ids {
        if let Some(senders) = map.get(&user_id) {
            let unread = queries::count_unread(&conn, user_id);
            let msg = serde_json::json!({
                "type": "new_warning",
                "warning_id": warning_id,
                "severity": severity,
                "title": title,
                "unread_count": unread,
            });
            let msg_str = msg.to_string();
            for sender in senders {
                let _ = sender.send(msg_str.clone());
            }
        }
    }
}

/// Send count update to a specific user.
pub fn send_count_update(conn_map: &ConnectionMap, pool: &DbPool, user_id: i64) {
    let conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return,
    };
    let unread = queries::count_unread(&conn, user_id);
    let msg = serde_json::json!({
        "type": "count_update",
        "unread_count": unread,
    });
    let msg_str = msg.to_string();
    let map = match conn_map.read() {
        Ok(m) => m,
        Err(_) => return,
    };
    if let Some(senders) = map.get(&user_id) {
        for sender in senders {
            let _ = sender.send(msg_str.clone());
        }
    }
}

/// WebSocket upgrade handler.
pub async fn ws_connect(
    req: HttpRequest,
    body: web::Payload,
    session: Session,
    conn_map: web::Data<ConnectionMap>,
) -> Result<HttpResponse, actix_web::Error> {
    let user_id = match get_user_id(&session) {
        Some(id) => id,
        None => return Ok(HttpResponse::Unauthorized().finish()),
    };

    let (response, mut ws_session, mut msg_stream) = actix_ws::handle(&req, body)?;

    let (tx, mut rx) = mpsc::unbounded_channel::<String>();

    // Register this connection
    {
        let mut map = conn_map.write().unwrap();
        map.entry(user_id).or_default().push(tx);
    }

    let conn_map_clone = conn_map.into_inner().clone();

    actix_web::rt::spawn(async move {
        loop {
            tokio::select! {
                Some(msg) = rx.recv() => {
                    if ws_session.text(msg).await.is_err() {
                        break;
                    }
                }
                Some(Ok(msg)) = msg_stream.recv() => {
                    match msg {
                        Message::Ping(bytes) => {
                            if ws_session.pong(&bytes).await.is_err() {
                                break;
                            }
                        }
                        Message::Close(_) => break,
                        Message::Text(_) => {
                            // Client messages handled via HTTP POST, not WS
                        }
                        _ => {}
                    }
                }
                else => break,
            }
        }

        // Clean up on disconnect
        if let Ok(mut map) = conn_map_clone.write() {
            if let Some(senders) = map.get_mut(&user_id) {
                senders.retain(|s| !s.is_closed());
                if senders.is_empty() {
                    map.remove(&user_id);
                }
            }
        }
    });

    Ok(response)
}
