use axum::{
    extract::{ws::{WebSocket, WebSocketUpgrade, Message}, Path, State},
    response::Response,
};
use futures_util::{sink::SinkExt, stream::StreamExt};
use tracing::{debug, error, info, warn};
use crate::server::AppState;

/// WebSocket handler for streaming status updates
pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    Path(execution_id): Path<String>,
    State(state): State<AppState>,
) -> Response {
    info!("WebSocket connection requested for execution: {}", execution_id);
    
    ws.on_upgrade(move |socket| handle_socket(socket, execution_id, state))
}

/// Handle individual WebSocket connection (unidirectional streaming)
async fn handle_socket(socket: WebSocket, execution_id: String, state: AppState) {
    info!("WebSocket connection established for execution: {}", execution_id);
    
    let (mut sender, _) = socket.split(); // Remove receiver - unidirectional only
    let mut status_receiver = state.status_broadcaster.subscribe();
    
    // Clone execution_id for use in the task
    let exec_id = execution_id.clone();
    
    // Only sender task - stream status events to client
    let sender_task = tokio::spawn(async move {
        while let Ok(event) = status_receiver.recv().await {
            // Only send events for this specific execution
            if event.execution_id == exec_id {
                match serde_json::to_string(&event) {
                    Ok(json) => {
                        if let Err(e) = sender.send(Message::Text(json.into())).await {
                            warn!("Failed to send WebSocket message: {}", e);
                            break;
                        }
                        debug!("Sent status event for execution {}", exec_id);
                    }
                    Err(e) => {
                        error!("Failed to serialize status event: {}", e);
                        break;
                    }
                }
            }
        }
        debug!("WebSocket sender task ended for execution: {}", exec_id);
    });
    
    // Wait for sender task to complete (connection closed or error)
    if let Err(e) = sender_task.await {
        warn!("WebSocket sender task error for execution {}: {}", execution_id, e);
    }
    
    info!("WebSocket connection closed for execution: {}", execution_id);
}
