use axum::{
    extract::{ws::{WebSocket, WebSocketUpgrade, Message}, Path, State},
    response::Response,
};
use futures_util::{sink::SinkExt, stream::StreamExt};
use tracing::{debug, error, info, warn, instrument};
use crate::server::AppState;

/// WebSocket handler for streaming status updates
#[instrument(skip(ws, state), fields(subscription_id = %subscription_id))]
pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    Path(subscription_id): Path<String>,
    State(state): State<AppState>,
) -> Response {
    info!(
        subscription_id = %subscription_id,
        "WebSocket connection upgrade requested"
    );
    
    ws.on_upgrade(move |socket| handle_socket(socket, subscription_id, state))
}

/// Handle individual WebSocket connection (unidirectional streaming)
#[instrument(skip(socket, state), fields(subscription_id = %subscription_id))]
async fn handle_socket(socket: WebSocket, subscription_id: String, state: AppState) {
    info!(
        subscription_id = %subscription_id,
        "WebSocket connection established, starting status stream"
    );
    
    let (mut sender, _) = socket.split(); // Remove receiver - unidirectional only
    
    // Connect to the subscription and get stream with buffered events replayed
    let mut status_receiver = match state.execution_manager.connect_subscription(&subscription_id).await {
        Some(receiver) => receiver,
        None => {
            error!("No subscription found for: {}", subscription_id);
            return;
        }
    };
    
    // Clone subscription_id for use in the task
    let sub_id = subscription_id.clone();
    
    // Only sender task - stream status events to client
    let sender_task = tokio::spawn(async move {
        while let Ok(event) = status_receiver.recv().await {
            match serde_json::to_string(&event) {
                Ok(json) => {
                    if let Err(e) = sender.send(Message::Text(json.into())).await {
                        warn!("Failed to send WebSocket message: {}", e);
                        break;
                    }
                    debug!("Sent status event for subscription {}", sub_id);
                }
                Err(e) => {
                    error!("Failed to serialize status event: {}", e);
                    break;
                }
            }
        }
        debug!("WebSocket sender task ended for subscription: {}", sub_id);
    });
    
    // Wait for sender task to complete (connection closed or error)
    if let Err(e) = sender_task.await {
        warn!("WebSocket sender task error for subscription {}: {}", subscription_id, e);
    }
    
    // Mark subscription as disconnected (but keep it alive for potential reconnection)
    state.execution_manager.disconnect_subscription(&subscription_id).await;
    
    info!("WebSocket connection closed for subscription: {} (subscription kept alive for reconnection)", subscription_id);
}
