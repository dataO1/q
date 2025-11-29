use axum::{
    extract::{ws::{WebSocket, WebSocketUpgrade, Message}, Path, State},
    response::Response,
};
use futures_util::{sink::SinkExt, stream::StreamExt};
use tracing::{debug, error, info, warn, instrument};
use crate::server::AppState;

/// WebSocket handler for streaming status updates
#[instrument(skip(ws, state), fields(conversation_id = %conversation_id))]
pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    Path(conversation_id): Path<String>,
    State(state): State<AppState>,
) -> Response {
    info!(
        conversation_id = %conversation_id,
        "WebSocket connection upgrade requested"
    );
    
    ws.on_upgrade(move |socket| handle_socket(socket, conversation_id, state))
}

/// Handle individual WebSocket connection (unidirectional streaming)
#[instrument(skip(socket, state), fields(conversation_id = %conversation_id))]
async fn handle_socket(socket: WebSocket, conversation_id: String, state: AppState) {
    info!(
        conversation_id = %conversation_id,
        "WebSocket connection established, starting status stream"
    );
    
    let (mut sender, _) = socket.split(); // Remove receiver - unidirectional only
    
    // Get the conversation stream from ExecutionManager
    let stream_result = state.execution_manager.get_conversation_stream(&conversation_id).await;
    let mut status_receiver = match stream_result {
        Some(receiver) => receiver,
        None => {
            error!("No conversation stream found for: {}", conversation_id);
            return;
        }
    };
    
    // Clone conversation_id for use in the task
    let conv_id = conversation_id.clone();
    
    // Only sender task - stream status events to client
    let sender_task = tokio::spawn(async move {
        while let Ok(event) = status_receiver.recv().await {
            match serde_json::to_string(&event) {
                Ok(json) => {
                    if let Err(e) = sender.send(Message::Text(json.into())).await {
                        warn!("Failed to send WebSocket message: {}", e);
                        break;
                    }
                    debug!("Sent status event for conversation {}", conv_id);
                }
                Err(e) => {
                    error!("Failed to serialize status event: {}", e);
                    break;
                }
            }
        }
        debug!("WebSocket sender task ended for conversation: {}", conv_id);
    });
    
    // Wait for sender task to complete (connection closed or error)
    if let Err(e) = sender_task.await {
        warn!("WebSocket sender task error for conversation {}: {}", conversation_id, e);
    }
    
    info!("WebSocket connection closed for conversation: {}", conversation_id);
}
